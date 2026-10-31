package com.cantos.theatre.adapters.playback

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.media.AudioAttributes
import android.media.AudioFocusRequest
import android.media.AudioManager
import android.media.MediaMetadata
import android.media.MediaPlayer
import android.media.PlaybackParams
import android.media.session.MediaSession
import android.media.session.PlaybackState
import android.net.Uri
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.PowerManager
import android.os.SystemClock
import com.cantos.theatre.domain.playback.PlaybackEngineCommand
import com.cantos.theatre.domain.playback.PlaybackEvent
import com.cantos.theatre.domain.playback.PlaybackFailure
import com.cantos.theatre.domain.playback.PlaybackIdentity
import com.cantos.theatre.domain.playback.PlaybackObservation
import com.cantos.theatre.ports.PlaybackLocation
import com.cantos.theatre.ports.StreamingMediaEngine
import com.cantos.theatre.shared.R
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.withContext

/**
 * Process-owned platform player. The reducer alone decides interruption resumption; this
 * adapter never resumes directly from audio focus or headset callbacks. Every platform call
 * and callback is main-looper confined. The 250 ms sampler reads MediaPlayer's actual position
 * and isPlaying acknowledgement; it never integrates elapsed wall time into a playhead.
 *
 * Signed locations are handed directly to MediaPlayer and are neither logged nor persisted.
 * MediaPlayer exposes generic I/O errors, not HTTP response status; grant expiry is resolved by
 * the authorizing wrapper, rather than guessed from a platform exception or its private URL.
 */
class AndroidMediaEngine(
    context: Context,
    private val onControl: (PlaybackEvent) -> Unit,
) : StreamingMediaEngine {
    private val context = context.applicationContext
    private val main = Handler(Looper.getMainLooper())
    private val observations = Channel<PlaybackObservation>(Channel.UNLIMITED)
    override val events = observations.receiveAsFlow()
    private val audio = this.context.getSystemService(Context.AUDIO_SERVICE) as AudioManager
    private val attributes = AudioAttributes.Builder()
        .setUsage(AudioAttributes.USAGE_MEDIA).setContentType(AudioAttributes.CONTENT_TYPE_SPEECH).build()
    private var player: MediaPlayer? = null
    private var current: PlaybackEngineCommand? = null
    private var loadEpoch = 0L
    private var prepared = false
    private var positionPending = false
    private var wantsPlay = false
    private var buffering = false
    private var closed = false
    private var reportingFailure = false
    private var seekInFlight: PlaybackEngineCommand? = null
    private var pendingSeek: PlaybackEngineCommand? = null
    private var startedIdentity: PlaybackIdentity? = null
    private var pausedIdentity: PlaybackIdentity? = null
    private var focusEpoch = 0L
    private var focusListener: AudioManager.OnAudioFocusChangeListener? = null
    private var focusRequest: AudioFocusRequest? = null
    private var ownsFocus = false
    private var focusInterrupted = false
    private var foreground: CantosPlaybackService? = null
    private var startingService = false
    private val mediaSession = MediaSession(this.context, "Cantos")
    private val noisy = object : BroadcastReceiver() {
        override fun onReceive(context: Context?, intent: Intent?) {
            if (!closed && current != null && intent?.action == AudioManager.ACTION_AUDIO_BECOMING_NOISY) {
                // A new explicit Play is required after route loss; reconnect never resumes.
                player?.takeIf { prepared }?.let { native -> platformCall { if (native.isPlaying) native.pause() } }
                onControl(PlaybackEvent.InterruptionBegan)
                onControl(PlaybackEvent.InterruptionEnded(resumeAllowed = false))
            }
        }
    }

    init {
        check(Looper.myLooper() == Looper.getMainLooper()) { "Create the media owner on the main looper" }
        check(CantosPlaybackService.owner == null) { "Cantos already owns a process media session" }
        CantosPlaybackService.owner = this
        mediaSession.setCallback(object : MediaSession.Callback() {
            override fun onPlay() = control(PlaybackEvent.PlayRequested)
            override fun onPause() = control(PlaybackEvent.PauseRequested)
            override fun onStop() = control(PlaybackEvent.PauseRequested)
            override fun onSeekTo(pos: Long) = control(PlaybackEvent.SeekRequested(pos))
            override fun onFastForward() = control(PlaybackEvent.SkipRequested(15_000))
            override fun onRewind() = control(PlaybackEvent.SkipRequested(-15_000))
            override fun onSetPlaybackSpeed(speed: Float) = control(PlaybackEvent.SpeedRequested((speed * 1000).toInt()))
        }, main)
        val launch = this.context.packageManager.getLaunchIntentForPackage(this.context.packageName)
        if (launch != null) mediaSession.setSessionActivity(PendingIntent.getActivity(
            this.context, 0, launch, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        ))
        if (Build.VERSION.SDK_INT >= 33) {
            this.context.registerReceiver(noisy, IntentFilter(AudioManager.ACTION_AUDIO_BECOMING_NOISY), Context.RECEIVER_NOT_EXPORTED)
        } else {
            @Suppress("DEPRECATION")
            this.context.registerReceiver(noisy, IntentFilter(AudioManager.ACTION_AUDIO_BECOMING_NOISY))
        }
    }

    private fun control(event: PlaybackEvent) {
        if (!closed && current != null) onControl(event)
    }

    override suspend fun load(command: PlaybackEngineCommand, location: PlaybackLocation) = withContext(Dispatchers.Main.immediate) {
        if (closed || (current?.identity?.generation ?: -1) > command.identity.generation) return@withContext
        releasePlayer()
        current = command
        wantsPlay = false
        positionPending = true
        scheduleSample()
        val epoch = ++loadEpoch
        if (!location.supportedByUrlOnlyDecoder) {
            // Refuse before decoder I/O or Now Playing publication, including a transient title.
            wantsPlay = false
            releasePlayer()
            emit(PlaybackEvent.EngineFailed(command.identity, PlaybackFailure.Unavailable))
            mediaSession.setMetadata(null)
            mediaSession.setPlaybackState(null)
            mediaSession.isActive = false
            foreground?.clear()
            foreground = null
            startingService = false
            current = null
            return@withContext
        }
        val native = MediaPlayer()
        player = native
        native.setAudioAttributes(attributes)
        native.setWakeMode(context, PowerManager.PARTIAL_WAKE_LOCK)
        native.setOnPreparedListener {
            if (isCurrent(native, epoch)) {
                prepared = true
                val latest = current ?: return@setOnPreparedListener
                emit(PlaybackEvent.EngineReady(latest.identity))
                queueSeek(latest)
            }
        }
        native.setOnSeekCompleteListener {
            if (isCurrent(native, epoch)) {
                val completed = seekInFlight
                seekInFlight = null
                val next = pendingSeek
                pendingSeek = null
                if (next != null) queueSeek(next)
                else if (completed?.identity == current?.identity) {
                    positionPending = false
                    samplePosition(native)
                    if (wantsPlay) beginPlayback() else observePause(native)
                    publishSession()
                }
            }
        }
        native.setOnCompletionListener {
            if (isCurrent(native, epoch) && seekInFlight == null && pendingSeek == null) {
                val latest = current ?: return@setOnCompletionListener
                wantsPlay = false
                main.removeCallbacks(sample)
                emit(PlaybackEvent.EngineEnded(latest.identity))
                abandonFocus()
                publishSession(PlaybackState.STATE_STOPPED)
                foreground?.demote()
            }
        }
        native.setOnInfoListener { _, what, _ ->
            if (isCurrent(native, epoch)) {
                when (what) {
                    MediaPlayer.MEDIA_INFO_BUFFERING_START -> {
                        buffering = true
                        current?.takeIf { wantsPlay }?.let { emit(PlaybackEvent.EngineBuffering(it.identity)) }
                        publishSession(PlaybackState.STATE_BUFFERING)
                    }
                    MediaPlayer.MEDIA_INFO_BUFFERING_END -> {
                        buffering = false
                        startedIdentity = null
                        observeRunning(native)
                    }
                }
            }
            false
        }
        native.setOnErrorListener { _, what, extra ->
            if (isCurrent(native, epoch)) fail(when {
                extra == MediaPlayer.MEDIA_ERROR_IO || extra == MediaPlayer.MEDIA_ERROR_TIMED_OUT -> PlaybackFailure.Network
                extra == MediaPlayer.MEDIA_ERROR_MALFORMED || extra == MediaPlayer.MEDIA_ERROR_UNSUPPORTED -> PlaybackFailure.Decode
                what == MediaPlayer.MEDIA_ERROR_SERVER_DIED -> PlaybackFailure.Unavailable
                else -> PlaybackFailure.Unavailable
            })
            true
        }
        platformCall {
            native.setDataSource(location.url)
            native.prepareAsync()
        }
    }

    override suspend fun prepare(command: PlaybackEngineCommand) = withContext(Dispatchers.Main.immediate) {
        if (!accept(command)) return@withContext
        wantsPlay = false
        if (prepared) queueSeek(command)
    }

    override suspend fun start(command: PlaybackEngineCommand) = withContext(Dispatchers.Main.immediate) {
        if (!accept(command)) return@withContext
        wantsPlay = true
        scheduleSample()
        // A foreground service exists before requesting focus on targetSdk 35, including
        // controls originating from the system while our Activity is in the background.
        ensureForeground()
        if (prepared) queueSeek(command)
    }

    override suspend fun pause(command: PlaybackEngineCommand) = withContext(Dispatchers.Main.immediate) {
        if (closed || (current?.identity?.generation ?: -1) > command.identity.generation) return@withContext
        if (command.clearSession) {
            wantsPlay = false
            releasePlayer()
            current = null
            mediaSession.setPlaybackState(null)
            mediaSession.setMetadata(null)
            mediaSession.isActive = false
            val service = foreground
            foreground = null
            startingService = false
            service?.clear()
            return@withContext
        }
        if (current?.source != command.source) {
            // A newly selected source can supersede an authorization still in flight. Stop
            // the loaded source immediately without assigning its callbacks to the new one.
            wantsPlay = false
            releasePlayer()
            foreground?.demote()
            return@withContext
        }
        if (current?.positionMs != command.positionMs) positionPending = true
        if (!accept(command)) return@withContext
        wantsPlay = false
        scheduleSample()
        player?.takeIf { prepared }?.let { native ->
            platformCall {
                if (native.isPlaying) native.pause()
                if (!positionPending) samplePosition(native)
                observePause(native)
            }
        }
        if (!focusInterrupted) {
            abandonFocus()
            foreground?.demote()
        }
        publishSession(PlaybackState.STATE_PAUSED)
    }

    override suspend fun seek(command: PlaybackEngineCommand) = withContext(Dispatchers.Main.immediate) {
        if (accept(command) && prepared) queueSeek(command)
    }

    override suspend fun setSpeed(command: PlaybackEngineCommand) = withContext(Dispatchers.Main.immediate) {
        if (!accept(command)) return@withContext
        // MediaPlayer.setPlaybackParams may start a prepared/paused player. Delay it until a
        // real Play rather than allowing a speed change to cause unexpected audio.
        player?.takeIf { prepared && wantsPlay && seekInFlight == null }?.let { native ->
            platformCall { if (native.isPlaying) native.playbackParams = PlaybackParams().setSpeed(command.speedPermille / 1000f) }
        }
        publishSession()
    }

    private fun accept(command: PlaybackEngineCommand): Boolean {
        val previous = current ?: return false
        if (closed || command.source != previous.source || command.identity.generation < previous.identity.generation) return false
        current = command
        return true
    }

    private fun queueSeek(command: PlaybackEngineCommand) {
        val native = player ?: return
        if (!prepared) return
        if (seekInFlight != null) { pendingSeek = command; return }
        seekInFlight = command
        positionPending = true
        scheduleSample()
        platformCall {
            if (native.isPlaying) native.pause()
            if (Build.VERSION.SDK_INT >= 26) native.seekTo(command.positionMs, MediaPlayer.SEEK_CLOSEST)
            else {
                @Suppress("DEPRECATION")
                native.seekTo(command.positionMs.coerceAtMost(Int.MAX_VALUE.toLong()).toInt())
            }
        }
    }

    private fun beginPlayback() {
        val native = player ?: return
        val command = current ?: return
        if (!wantsPlay || !prepared || seekInFlight != null || foreground == null) return
        platformCall {
            if (!requestFocus()) return@platformCall
            native.playbackParams = PlaybackParams().setSpeed(command.speedPermille / 1000f)
            native.start()
            observeRunning(native)
        }
    }

    private val sample = object : Runnable {
        override fun run() {
            if (closed) return
            player?.takeIf { prepared && seekInFlight == null }?.let { native ->
                platformCall {
                    if (!positionPending) samplePosition(native)
                    if (wantsPlay) observeRunning(native) else observePause(native)
                }
            }
            if (player != null && (wantsPlay || !prepared || seekInFlight != null ||
                    (!positionPending && pausedIdentity != current?.identity))) {
                main.postDelayed(this, 250)
            }
        }
    }

    private fun scheduleSample() {
        main.removeCallbacks(sample)
        main.post(sample)
    }

    private fun samplePosition(native: MediaPlayer) {
        val command = current ?: return
        val position = native.currentPosition.toLong().coerceIn(0, command.source.durationMs)
        current = command.copy(positionMs = position)
        emit(PlaybackEvent.EnginePosition(command.identity, position))
    }

    private fun observeRunning(native: MediaPlayer) {
        val command = current ?: return
        if (wantsPlay && !buffering && native.isPlaying && startedIdentity != command.identity) {
            startedIdentity = command.identity
            pausedIdentity = null
            emit(PlaybackEvent.EngineStarted(command.identity))
            publishSession(PlaybackState.STATE_PLAYING)
        }
    }

    private fun observePause(native: MediaPlayer) {
        val command = current ?: return
        if (!native.isPlaying && pausedIdentity != command.identity) {
            pausedIdentity = command.identity
            startedIdentity = null
            emit(PlaybackEvent.EnginePaused(command.identity))
            publishSession(PlaybackState.STATE_PAUSED)
        }
    }

    private fun ensureForeground() {
        if (foreground != null) { platformCall { foreground?.promote() }; return }
        if (startingService) return
        startingService = true
        platformCall {
            val intent = Intent(context, CantosPlaybackService::class.java)
            if (Build.VERSION.SDK_INT >= 26) context.startForegroundService(intent) else context.startService(intent)
        }
    }

    internal fun attach(service: CantosPlaybackService) {
        if (closed || current == null || CantosPlaybackService.owner !== this) {
            startingService = false
            service.stopSelf()
            return
        }
        foreground = service
        startingService = false
        platformCall {
            service.promote()
            if (wantsPlay) beginPlayback() else service.demote()
        }
    }

    internal fun detach(service: CantosPlaybackService) {
        if (foreground !== service) return
        foreground = null
        if (!closed && wantsPlay) {
            player?.takeIf { prepared }?.let { native -> platformCall { if (native.isPlaying) native.pause() } }
            onControl(PlaybackEvent.PauseRequested)
        }
    }

    internal fun acceptsRemote(intent: Intent?): Boolean {
        val action = intent?.action ?: return true // Engine-owned foreground startup.
        return (action == ACTION_PLAY || action == ACTION_PAUSE) &&
            intent.getLongExtra(EXTRA_ITEM_TOKEN, -1) == loadEpoch &&
            intent.getLongExtra(EXTRA_GENERATION, -1) == current?.identity?.generation
    }

    internal fun remote(intent: Intent?) {
        val action = intent?.action ?: return
        if (!acceptsRemote(intent)) return
        when (action) {
            ACTION_PLAY -> control(PlaybackEvent.PlayRequested)
            ACTION_PAUSE -> control(PlaybackEvent.PauseRequested)
        }
    }

    private fun requestFocus(): Boolean {
        if (ownsFocus) return true
        val epoch = ++focusEpoch
        val listener = AudioManager.OnAudioFocusChangeListener { change ->
            if (closed || epoch != focusEpoch || current == null) return@OnAudioFocusChangeListener
            when (change) {
                AudioManager.AUDIOFOCUS_GAIN -> {
                    ownsFocus = true
                    if (focusInterrupted) {
                        // Releasing a regained focus prevents a listener pause made during
                        // the interruption from retaining focus forever. Only a reducer Start
                        // reacquires it; the OS callback itself never resumes playback.
                        abandonFocus()
                        foreground?.demote()
                        onControl(PlaybackEvent.InterruptionEnded(resumeAllowed = true))
                    }
                }
                AudioManager.AUDIOFOCUS_LOSS_TRANSIENT, AudioManager.AUDIOFOCUS_LOSS_TRANSIENT_CAN_DUCK -> {
                    ownsFocus = false
                    focusInterrupted = true
                    player?.takeIf { prepared }?.let { native -> platformCall { if (native.isPlaying) native.pause() } }
                    onControl(PlaybackEvent.InterruptionBegan)
                }
                AudioManager.AUDIOFOCUS_LOSS -> {
                    ownsFocus = false
                    focusInterrupted = true
                    player?.takeIf { prepared }?.let { native -> platformCall { if (native.isPlaying) native.pause() } }
                    onControl(PlaybackEvent.InterruptionBegan)
                    onControl(PlaybackEvent.InterruptionEnded(resumeAllowed = false))
                    abandonFocus()
                    foreground?.demote()
                }
            }
        }
        focusListener = listener
        val outcome = if (Build.VERSION.SDK_INT >= 26) {
            val request = AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN)
                .setAudioAttributes(attributes).setWillPauseWhenDucked(true)
                .setOnAudioFocusChangeListener(listener, main).build()
            focusRequest = request
            audio.requestAudioFocus(request)
        } else {
            @Suppress("DEPRECATION")
            audio.requestAudioFocus(listener, AudioManager.STREAM_MUSIC, AudioManager.AUDIOFOCUS_GAIN)
        }
        ownsFocus = outcome == AudioManager.AUDIOFOCUS_REQUEST_GRANTED
        if (!ownsFocus) {
            // No auto-retry or delayed audio. The reducer retains a distinct interrupted state.
            focusInterrupted = true
            onControl(PlaybackEvent.InterruptionBegan)
            onControl(PlaybackEvent.InterruptionEnded(resumeAllowed = false))
        }
        return ownsFocus
    }

    private fun abandonFocus() {
        ++focusEpoch
        if (Build.VERSION.SDK_INT >= 26) focusRequest?.let(audio::abandonAudioFocusRequest)
        else {
            @Suppress("DEPRECATION")
            focusListener?.let(audio::abandonAudioFocus)
        }
        focusRequest = null
        focusListener = null
        ownsFocus = false
        focusInterrupted = false
    }

    private fun publishSession(forcedState: Int? = null) {
        val command = current ?: return
        val native = player
        val running = prepared && native != null && wantsPlay && !buffering && seekInFlight == null &&
            (forcedState == null || forcedState == PlaybackState.STATE_PLAYING) && native.isPlaying
        val state = forcedState ?: if (running) PlaybackState.STATE_PLAYING else PlaybackState.STATE_PAUSED
        val position = if (prepared && native != null) native.currentPosition.toLong() else command.positionMs
        mediaSession.isActive = true
        mediaSession.setMetadata(MediaMetadata.Builder().putString(MediaMetadata.METADATA_KEY_TITLE, command.source.title.ifBlank { "Cantos" })
            .putString(MediaMetadata.METADATA_KEY_ALBUM, command.source.workTitle)
            .putString(MediaMetadata.METADATA_KEY_MEDIA_ID, command.source.audio?.publicationId ?: command.source.workId)
            .putLong(MediaMetadata.METADATA_KEY_DURATION, command.source.durationMs).build())
        mediaSession.setPlaybackState(PlaybackState.Builder().setActions(
            PlaybackState.ACTION_PLAY or PlaybackState.ACTION_PAUSE or PlaybackState.ACTION_PLAY_PAUSE or
                PlaybackState.ACTION_SEEK_TO or PlaybackState.ACTION_REWIND or PlaybackState.ACTION_FAST_FORWARD or
                PlaybackState.ACTION_SET_PLAYBACK_SPEED,
        ).setState(state, position, if (running) command.speedPermille / 1000f else 0f, SystemClock.elapsedRealtime()).build())
        platformCall { foreground?.refresh() }
    }

    internal fun notification(service: Service): Notification {
        val running = startedIdentity == current?.identity && wantsPlay && !buffering
        fun action(name: String): PendingIntent = PendingIntent.getService(context, if (name == ACTION_PLAY) 1 else 2,
            Intent(context, CantosPlaybackService::class.java).setAction(name)
                .setData(Uri.parse("cantos-media://item/$loadEpoch/${current?.identity?.generation}/$name"))
                .putExtra(EXTRA_ITEM_TOKEN, loadEpoch).putExtra(EXTRA_GENERATION, current?.identity?.generation ?: -1),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
        val builder = if (Build.VERSION.SDK_INT >= 26) Notification.Builder(service, CHANNEL) else {
            @Suppress("DEPRECATION")
            Notification.Builder(service)
        }
        val icon = if (running) android.R.drawable.ic_media_pause else android.R.drawable.ic_media_play
        val label = service.getString(if (running) R.string.cantos_media_pause else R.string.cantos_media_play)
        return builder.setSmallIcon(android.R.drawable.ic_media_play)
            .setContentTitle(current?.source?.title?.takeIf { it.isNotBlank() } ?: "Cantos")
            .setContentText(current?.source?.workTitle?.takeIf { it.isNotBlank() })
            .setVisibility(Notification.VISIBILITY_PUBLIC).setOnlyAlertOnce(true).setOngoing(running)
            .setContentIntent(mediaSession.controller.sessionActivity)
            .addAction(Notification.Action.Builder(icon, label, action(if (running) ACTION_PAUSE else ACTION_PLAY)).build())
            .setStyle(Notification.MediaStyle().setMediaSession(mediaSession.sessionToken).setShowActionsInCompactView(0)).build()
    }

    private fun emit(event: PlaybackObservation) { if (!closed) observations.trySend(event) }
    private fun isCurrent(native: MediaPlayer, epoch: Long): Boolean = !closed && player === native && epoch == loadEpoch

    private inline fun platformCall(action: () -> Unit) {
        try { action() }
        catch (_: java.io.IOException) { fail(PlaybackFailure.Network) }
        catch (_: SecurityException) { fail(PlaybackFailure.Unavailable) }
        catch (_: IllegalArgumentException) { fail(PlaybackFailure.Decode) }
        catch (_: IllegalStateException) { fail(PlaybackFailure.Unavailable) }
    }

    private fun fail(failure: PlaybackFailure) {
        if (closed || reportingFailure) return
        val identity = current?.identity ?: return
        reportingFailure = true
        try {
            wantsPlay = false
            releasePlayer()
            emit(PlaybackEvent.EngineFailed(identity, failure))
            publishSession(PlaybackState.STATE_ERROR)
            foreground?.demote()
        } finally { reportingFailure = false }
    }

    private fun releasePlayer() {
        main.removeCallbacks(sample)
        ++loadEpoch
        val native = player
        player = null
        prepared = false
        positionPending = false
        buffering = false
        seekInFlight = null
        pendingSeek = null
        startedIdentity = null
        pausedIdentity = null
        native?.release()
        abandonFocus()
    }

    override fun close() {
        if (Looper.myLooper() != Looper.getMainLooper()) { main.post { close() }; return }
        if (closed) return
        closed = true
        main.removeCallbacks(sample)
        releasePlayer()
        context.unregisterReceiver(noisy)
        mediaSession.release()
        if (CantosPlaybackService.owner === this) CantosPlaybackService.owner = null
        foreground?.stopSelf()
        foreground = null
        observations.close()
    }

    internal companion object {
        const val CHANNEL = "cantos-playback"
        const val ACTION_PLAY = "com.cantos.theatre.PLAY"
        const val ACTION_PAUSE = "com.cantos.theatre.PAUSE"
        const val EXTRA_ITEM_TOKEN = "cantos.item"
        const val EXTRA_GENERATION = "cantos.generation"
    }
}

/** A non-sticky lifecycle anchor, never an independent engine or a cold-start autoplay path. */
class CantosPlaybackService : Service() {
    private var promoted = false
    override fun onCreate() {
        super.onCreate()
        if (Build.VERSION.SDK_INT >= 26) {
            val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            manager.createNotificationChannel(NotificationChannel(AndroidMediaEngine.CHANNEL, "Cantos", NotificationManager.IMPORTANCE_LOW))
        }
    }
    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val engine = owner
        if (engine == null) { stopSelf(); return START_NOT_STICKY }
        if (!engine.acceptsRemote(intent)) {
            if (!promoted) stopSelf(startId)
            return START_NOT_STICKY
        }
        engine.attach(this)
        engine.remote(intent)
        return START_NOT_STICKY
    }
    internal fun promote() {
        val engine = owner ?: return
        startForeground(NOTIFICATION, engine.notification(this))
        promoted = true
    }
    internal fun refresh() {
        val engine = owner ?: return
        (getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager).notify(NOTIFICATION, engine.notification(this))
    }
    internal fun demote() {
        if (!promoted) return
        @Suppress("DEPRECATION")
        stopForeground(false)
        promoted = false
    }
    internal fun clear() {
        @Suppress("DEPRECATION")
        stopForeground(true)
        promoted = false
        (getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager).cancel(NOTIFICATION)
        stopSelf()
    }
    override fun onDestroy() {
        owner?.detach(this)
        (getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager).cancel(NOTIFICATION)
        super.onDestroy()
    }
    override fun onBind(intent: Intent?): IBinder? = null
    companion object {
        internal var owner: AndroidMediaEngine? = null
        private const val NOTIFICATION = 1401
    }
}

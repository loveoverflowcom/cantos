import AVFoundation
import MediaPlayer
import Foundation
import CantosTheatre

/// One process-owned AVPlayer. KVO and periodic callbacks report observed engine facts;
/// no timer estimates playback position. The Kotlin reducer alone decides whether to resume.
/// The driver activates AVAudioSession only after explicit Play, never on load/cold restore.
final class AvPlayerDriver: NSObject, IosMediaDriver {
    private static weak var active: AvPlayerDriver?
    private let player = AVPlayer()
    private var listener: IosMediaListener?
    private var command: PlaybackEngineCommand?
    private var token: Int64 = 0
    private var wantsPlay = false
    private var ready = false
    private var seeking = false
    private var positionPending = false
    private var seekSerial: UInt64 = 0
    private var interrupted = false
    private var closed = false
    private var statusObservation: NSKeyValueObservation?
    private var controlObservation: NSKeyValueObservation?
    private var timeObserver: Any?
    private var notifications: [NSObjectProtocol] = []
    private var itemNotifications: [NSObjectProtocol] = []
    private var remoteTargets: [(MPRemoteCommand, Any)] = []
    private var lastStatus: IosMediaFact?
    private var lastStatusGeneration: Int64 = -1

    func attach(listener: IosMediaListener) {
        precondition(Thread.isMainThread)
        precondition(self.listener == nil, "The native media driver already has an owner")
        precondition(Self.active == nil || Self.active === self, "Cantos already owns a process media session")
        Self.active = self
        self.listener = listener
        let center = NotificationCenter.default
        notifications.append(center.addObserver(forName: AVAudioSession.interruptionNotification,
            object: AVAudioSession.sharedInstance(), queue: .main) { [weak self] note in self?.interruption(note) })
        notifications.append(center.addObserver(forName: AVAudioSession.routeChangeNotification,
            object: AVAudioSession.sharedInstance(), queue: .main) { [weak self] note in
                guard let self, !self.closed, self.command != nil,
                    let reason = note.userInfo?[AVAudioSessionRouteChangeReasonKey] as? UInt,
                    reason == AVAudioSession.RouteChangeReason.oldDeviceUnavailable.rawValue else { return }
                self.player.pause()
                self.listener?.control(control: .interruptionBegan, value: 0)
                self.listener?.control(control: .interruptionPause, value: 0)
            })
        controlObservation = player.observe(\.timeControlStatus, options: [.new]) { [weak self] _, _ in
            // AVFoundation KVO delivery is not assumed to be main-thread delivery.
            DispatchQueue.main.async { self?.reportTimeControl() }
        }
        timeObserver = player.addPeriodicTimeObserver(forInterval: CMTime(seconds: 0.25, preferredTimescale: 1000),
            queue: .main) { [weak self] _ in self?.position() }
        installRemoteCommands()
    }

    func load(token: Int64, command: PlaybackEngineCommand, location: PlaybackLocation) {
        precondition(Thread.isMainThread)
        guard !closed else { return }
        invalidateItem()
        self.token = token
        self.command = command
        wantsPlay = false
        ready = false
        positionPending = true
        lastStatus = nil
        guard location.supportedByUrlOnlyDecoder else {
            player.replaceCurrentItem(with: nil)
            report(.unavailable)
            self.command = nil
            MPNowPlayingInfoCenter.default().nowPlayingInfo = nil
            deactivateAudio()
            return
        }
        guard let url = URL(string: location.url), ["https", "http"].contains(url.scheme?.lowercased() ?? "") else {
            fail(.unavailable); return
        }
        // The location is ephemeral. Never print AVFoundation errors/userInfo, whose contents
        // can contain the signed media URL, or write a URL to Now Playing/checkpoints.
        let item = AVPlayerItem(url: url)
        player.replaceCurrentItem(with: item)
        statusObservation = item.observe(\.status, options: [.initial, .new]) { [weak self, weak item] _, _ in
            DispatchQueue.main.async {
                guard let self, let item, self.isCurrent(item, token: token) else { return }
                switch item.status {
                case .readyToPlay:
                    self.ready = true
                    self.report(.ready)
                    self.seekToCommand()
                case .failed:
                    self.fail(self.classify(item.error))
                case .unknown: break
                @unknown default: self.fail(.unavailable)
                }
            }
        }
        let center = NotificationCenter.default
        itemNotifications.append(center.addObserver(forName: .AVPlayerItemDidPlayToEndTime, object: item, queue: .main) { [weak self, weak item] _ in
            guard let self, let item, self.isCurrent(item, token: token), !self.seeking else { return }
            self.wantsPlay = false
            self.position()
            self.report(.ended)
            self.nowPlaying()
            self.deactivateAudio()
        })
        itemNotifications.append(center.addObserver(forName: .AVPlayerItemFailedToPlayToEndTime, object: item, queue: .main) { [weak self, weak item] _ in
            guard let self, let item, self.isCurrent(item, token: token) else { return }
            self.fail(self.classify(item.error))
        })
        itemNotifications.append(center.addObserver(forName: .AVPlayerItemPlaybackStalled, object: item, queue: .main) { [weak self, weak item] _ in
            guard let self, let item, self.isCurrent(item, token: token), self.wantsPlay else { return }
            self.report(.buffering)
        })
    }

    func update(token: Int64, command: PlaybackEngineCommand, operation: IosMediaOperation) {
        precondition(Thread.isMainThread)
        guard !closed, self.token == token, self.command != nil else { return }
        if command.clearSession {
            invalidateItem()
            player.replaceCurrentItem(with: nil)
            self.command = nil
            wantsPlay = false
            ready = false
            positionPending = false
            lastStatus = nil
            MPNowPlayingInfoCenter.default().nowPlayingInfo = nil
            deactivateAudio()
            return
        }
        let previousPosition = self.command?.positionMs
        self.command = command
        switch operation {
        case .prepare:
            wantsPlay = false
            player.pause()
            if ready { seekToCommand() }
        case .start:
            wantsPlay = true
            if ready { seekToCommand() }
        case .pause:
            wantsPlay = false
            if previousPosition != command.positionMs || seeking { positionPending = true }
            player.currentItem?.cancelPendingSeeks()
            seekSerial &+= 1
            seeking = false
            player.pause()
            reportTimeControl()
            position()
            nowPlaying()
            if !interrupted { deactivateAudio() }
        case .seek:
            if ready { seekToCommand() }
        case .speed:
            // Setting rate on a paused AVPlayer starts playback. Retain the desired rate in
            // the command and apply it only when the player is already running or on Play.
            if wantsPlay && !seeking && player.timeControlStatus == .playing {
                player.rate = Float(command.speedPermille) / 1000
            }
            nowPlaying()
        default:
            fail(.unavailable)
        }
    }

    private func seekToCommand() {
        guard let command, ready, let item = player.currentItem else { return }
        seeking = true
        positionPending = true
        seekSerial &+= 1
        let serial = seekSerial
        let itemToken = token
        let generation = command.identity.generation
        player.pause()
        player.seek(to: CMTime(value: command.positionMs, timescale: 1000), toleranceBefore: .zero, toleranceAfter: .zero) { [weak self, weak item] finished in
            DispatchQueue.main.async {
                guard let self, let item, self.isCurrent(item, token: itemToken), serial == self.seekSerial,
                    generation == self.command?.identity.generation else { return }
                self.seeking = false
                guard finished else { self.fail(.unavailable); return }
                self.positionPending = false
                self.position()
                if self.wantsPlay { self.beginPlayback() }
                else { self.reportTimeControl(); self.nowPlaying() }
            }
        }
    }

    private func beginPlayback() {
        guard ready, wantsPlay, !seeking, !interrupted, let command else { return }
        do {
            let audio = AVAudioSession.sharedInstance()
            try audio.setCategory(.playback, mode: .spokenAudio, options: [])
            try audio.setActive(true)
            player.playImmediately(atRate: Float(command.speedPermille) / 1000)
            // timeControlStatus/KVO acknowledges running. A call to Play is not a start fact.
            reportTimeControl()
        } catch { fail(.unavailable) }
    }

    private func reportTimeControl() {
        guard !closed, ready, !seeking, command != nil else { return }
        switch player.timeControlStatus {
        case .playing: if wantsPlay && !interrupted { report(.started) }
        case .waitingToPlayAtSpecifiedRate: if wantsPlay { report(.buffering) }
        case .paused: report(.paused)
        @unknown default: fail(.unavailable)
        }
        nowPlaying()
    }

    private func position() {
        guard !closed, ready, !seeking, !positionPending, let command else { return }
        let seconds = player.currentTime().seconds
        guard seconds.isFinite, seconds >= 0 else { return }
        let bounded = min(seconds * 1000, Double(command.source.durationMs))
        listener?.observed(token: token, generation: command.identity.generation, fact: .position, positionMs: Int64(bounded))
    }

    private func report(_ fact: IosMediaFact) {
        guard !closed, let command else { return }
        if fact == lastStatus && command.identity.generation == lastStatusGeneration { return }
        lastStatus = fact
        lastStatusGeneration = command.identity.generation
        listener?.observed(token: token, generation: command.identity.generation, fact: fact, positionMs: 0)
    }

    private func isCurrent(_ item: AVPlayerItem, token: Int64) -> Bool {
        !closed && self.token == token && player.currentItem === item
    }

    private func interruption(_ notification: Notification) {
        guard !closed, command != nil,
            let raw = notification.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt,
            let type = AVAudioSession.InterruptionType(rawValue: raw) else { return }
        switch type {
        case .began:
            interrupted = true
            player.pause()
            listener?.control(control: .interruptionBegan, value: 0)
        case .ended:
            interrupted = false
            let rawOptions = notification.userInfo?[AVAudioSessionInterruptionOptionKey] as? UInt ?? 0
            let resume = AVAudioSession.InterruptionOptions(rawValue: rawOptions).contains(.shouldResume)
            listener?.control(control: resume ? .interruptionResume : .interruptionPause, value: 0)
        @unknown default:
            listener?.control(control: .interruptionPause, value: 0)
        }
    }

    private func installRemoteCommands() {
        let center = MPRemoteCommandCenter.shared()
        func register(_ command: MPRemoteCommand, _ handler: @escaping (MPRemoteCommandEvent) -> MPRemoteCommandHandlerStatus) {
            command.isEnabled = true
            remoteTargets.append((command, command.addTarget(handler: handler)))
        }
        register(center.playCommand) { [weak self] _ in self?.remote(.play) ?? .commandFailed }
        register(center.pauseCommand) { [weak self] _ in self?.remote(.pause) ?? .commandFailed }
        register(center.stopCommand) { [weak self] _ in self?.remote(.pause) ?? .commandFailed }
        register(center.togglePlayPauseCommand) { [weak self] _ in
            guard let self else { return .commandFailed }
            return self.remote(self.wantsPlay ? .pause : .play)
        }
        register(center.changePlaybackPositionCommand) { [weak self] event in
            guard let self, let command = self.command, let event = event as? MPChangePlaybackPositionCommandEvent,
                event.positionTime.isFinite, event.positionTime >= 0,
                event.positionTime <= Double(command.source.durationMs) / 1000 else { return .commandFailed }
            return self.remote(.seek, value: Int64(event.positionTime * 1000))
        }
        center.changePlaybackRateCommand.supportedPlaybackRates = [0.75, 1, 1.25, 1.5, 2]
        register(center.changePlaybackRateCommand) { [weak self] event in
            guard let event = event as? MPChangePlaybackRateCommandEvent, event.playbackRate.isFinite,
                [Float(0.75), 1, 1.25, 1.5, 2].contains(event.playbackRate) else { return .commandFailed }
            return self?.remote(.speed, value: Int64(event.playbackRate * 1000)) ?? .commandFailed
        }
        center.nextTrackCommand.isEnabled = false
        center.previousTrackCommand.isEnabled = false
    }

    private func remote(_ control: IosMediaControl, value: Int64 = 0) -> MPRemoteCommandHandlerStatus {
        guard !closed, let command else { return .noActionableNowPlayingItem }
        let itemToken = token
        let generation = command.identity.generation
        DispatchQueue.main.async { [weak self] in
            guard let self, !self.closed, self.token == itemToken,
                self.command?.identity.generation == generation else { return }
            self.listener?.control(control: control, value: value)
        }
        return .success
    }

    private func nowPlaying() {
        guard let command else { return }
        let seconds = player.currentTime().seconds
        let title = command.source.title.trimmingCharacters(in: .whitespacesAndNewlines)
        MPNowPlayingInfoCenter.default().nowPlayingInfo = [
            MPMediaItemPropertyTitle: title.isEmpty ? "Cantos" : title,
            MPMediaItemPropertyAlbumTitle: command.source.workTitle,
            MPMediaItemPropertyPlaybackDuration: Double(command.source.durationMs) / 1000,
            MPNowPlayingInfoPropertyElapsedPlaybackTime: seconds.isFinite ? max(0, seconds) : 0,
            MPNowPlayingInfoPropertyPlaybackRate:
                player.timeControlStatus == .playing && wantsPlay && !interrupted && !seeking ? player.rate : 0
        ]
    }

    private func classify(_ error: Error?) -> IosMediaFact {
        guard let error = error as NSError? else { return .unavailable }
        if error.domain == NSURLErrorDomain { return .network }
        if error.domain == AVFoundationErrorDomain {
            switch error.code {
            case AVError.decoderNotFound.rawValue, AVError.decodeFailed.rawValue,
                AVError.fileFormatNotRecognized.rawValue: return .decode
            default: return .unavailable
            }
        }
        return .unavailable
    }

    private func fail(_ reason: IosMediaFact) {
        wantsPlay = false
        player.pause()
        report(reason)
        nowPlaying()
        deactivateAudio()
    }

    private func deactivateAudio() {
        // Deactivation can fail when another OS transition already deactivated the session.
        // It cannot change the accepted playback state, and never triggers automatic playback.
        try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
    }

    private func invalidateItem() {
        player.pause()
        player.currentItem?.cancelPendingSeeks()
        seekSerial &+= 1
        seeking = false
        statusObservation?.invalidate()
        statusObservation = nil
        itemNotifications.forEach(NotificationCenter.default.removeObserver)
        itemNotifications.removeAll()
    }

    func close() {
        precondition(Thread.isMainThread)
        guard !closed else { return }
        closed = true
        invalidateItem()
        player.replaceCurrentItem(with: nil)
        controlObservation?.invalidate()
        controlObservation = nil
        if let timeObserver { player.removeTimeObserver(timeObserver) }
        timeObserver = nil
        notifications.forEach(NotificationCenter.default.removeObserver)
        notifications.removeAll()
        remoteTargets.forEach { $0.0.removeTarget($0.1) }
        remoteTargets.removeAll()
        MPNowPlayingInfoCenter.default().nowPlayingInfo = nil
        deactivateAudio()
        listener = nil
        command = nil
        if Self.active === self { Self.active = nil }
    }
}

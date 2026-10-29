package com.cantos.theatre.adapters.image

import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.toComposeImageBitmap
import kotlinx.cinterop.ExperimentalForeignApi
import kotlinx.cinterop.addressOf
import kotlinx.cinterop.convert
import kotlinx.cinterop.reinterpret
import kotlinx.cinterop.usePinned
import org.jetbrains.skia.ColorAlphaType
import org.jetbrains.skia.ColorType
import org.jetbrains.skia.Image
import org.jetbrains.skia.ImageInfo
import platform.CoreFoundation.CFDataCreate
import platform.CoreFoundation.CFRelease
import platform.CoreGraphics.CGBitmapContextCreate
import platform.CoreGraphics.CGColorSpaceCreateDeviceRGB
import platform.CoreGraphics.CGColorSpaceRelease
import platform.CoreGraphics.CGContextDrawImage
import platform.CoreGraphics.CGContextRelease
import platform.CoreGraphics.CGImageGetHeight
import platform.CoreGraphics.CGImageGetWidth
import platform.CoreGraphics.CGImageRelease
import platform.CoreGraphics.CGRectMake
import platform.CoreGraphics.CGImageAlphaInfo.kCGImageAlphaPremultipliedLast
import platform.CoreGraphics.kCGBitmapByteOrder32Big
import platform.ImageIO.CGImageSourceCreateImageAtIndex
import platform.ImageIO.CGImageSourceCreateWithData
import platform.ImageIO.CGImageSourceGetCount

/** ImageIO supplies the OS AVIF codec; unsupported OS versions return the native placeholder. */
@OptIn(ExperimentalForeignApi::class)
internal actual fun decodeBundledAvifPixels(bytes: ByteArray): ImageBitmap? {
    if (bytes.isEmpty() || bytes.size > 8_388_608) return null
    val data = bytes.usePinned { CFDataCreate(null, it.addressOf(0).reinterpret(), bytes.size.convert()) } ?: return null
    try {
        val source = CGImageSourceCreateWithData(data, null) ?: return null
        try {
            if (CGImageSourceGetCount(source) != 1UL) return null
            val image = CGImageSourceCreateImageAtIndex(source, 0UL, null) ?: return null
            try {
                val widthPx = CGImageGetWidth(image)
                val heightPx = CGImageGetHeight(image)
                if (widthPx !in 1UL..4096UL || heightPx !in 1UL..4096UL) return null
                val width = widthPx.toInt()
                val height = heightPx.toInt()
                val pixels = ByteArray(width * height * 4)
                val colorSpace = CGColorSpaceCreateDeviceRGB() ?: return null
                try {
                    return pixels.usePinned { pinned ->
                        val context = CGBitmapContextCreate(pinned.addressOf(0), width.convert(), height.convert(),
                            8UL, (width * 4).convert(), colorSpace, kCGImageAlphaPremultipliedLast.value or kCGBitmapByteOrder32Big) ?: return null
                        try {
                            CGContextDrawImage(context, CGRectMake(0.0, 0.0, width.toDouble(), height.toDouble()), image)
                            Image.makeRaster(ImageInfo(width, height, ColorType.RGBA_8888, ColorAlphaType.PREMUL),
                                pixels, width * 4).toComposeImageBitmap()
                        } finally {
                            CGContextRelease(context)
                        }
                    }
                } finally {
                    CGColorSpaceRelease(colorSpace)
                }
            } finally {
                CGImageRelease(image)
            }
        } finally {
            CFRelease(source)
        }
    } finally {
        CFRelease(data)
    }
}

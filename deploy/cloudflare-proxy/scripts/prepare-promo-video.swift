import Foundation
import AVFoundation
let source = URL(fileURLWithPath: CommandLine.arguments[1])
let destination = URL(fileURLWithPath: CommandLine.arguments[2])
guard let export = AVAssetExportSession(asset: AVURLAsset(url: source), presetName: AVAssetExportPresetPassthrough) else { fatalError("Cannot create MP4 export") }
export.shouldOptimizeForNetworkUse = true
export.outputURL = destination
export.outputFileType = .mp4
await export.export()
if export.status != .completed { throw export.error ?? NSError(domain: "PromoExport", code: 1) }
print("Created streaming MP4 without re-encoding the video.")

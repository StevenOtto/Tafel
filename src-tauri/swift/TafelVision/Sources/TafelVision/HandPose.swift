import Foundation
import SwiftRs
import Vision

/// One recognised joint. Coordinates are frame fractions with the origin at
/// the top-left (image convention) — Vision reports bottom-left, flipped here
/// so the webview can draw without thinking about it.
struct Point: Encodable {
    let x: Double
    let y: Double
    let confidence: Double
}

struct Hand: Encodable {
    let chirality: String
    let points: [String: Point]
}

struct Detection: Encodable {
    let hands: [Hand]
    let error: String?
}

/// Readable names for Vision's 21 hand joints, in the order Vision defines.
private let jointNames: [(VNHumanHandPoseObservation.JointName, String)] = [
    (.wrist, "wrist"),
    (.thumbCMC, "thumbCmc"), (.thumbMP, "thumbMp"), (.thumbIP, "thumbIp"), (.thumbTip, "thumbTip"),
    (.indexMCP, "indexMcp"), (.indexPIP, "indexPip"), (.indexDIP, "indexDip"), (.indexTip, "indexTip"),
    (.middleMCP, "middleMcp"), (.middlePIP, "middlePip"), (.middleDIP, "middleDip"), (.middleTip, "middleTip"),
    (.ringMCP, "ringMcp"), (.ringPIP, "ringPip"), (.ringDIP, "ringDip"), (.ringTip, "ringTip"),
    (.littleMCP, "littleMcp"), (.littlePIP, "littlePip"), (.littleDIP, "littleDip"), (.littleTip, "littleTip"),
]

private func chiralityName(_ observation: VNHumanHandPoseObservation) -> String {
    guard #available(macOS 14.0, *) else { return "unknown" }
    switch observation.chirality {
    case .left: return "left"
    case .right: return "right"
    default: return "unknown"
    }
}

private func encode(_ detection: Detection) -> SRString {
    let encoder = JSONEncoder()
    guard let data = try? encoder.encode(detection),
          let json = String(data: data, encoding: .utf8) else {
        return SRString("{\"hands\":[],\"error\":\"encoding failed\"}")
    }
    return SRString(json)
}

/// Runs Vision's hand-pose request on an encoded image (JPEG or PNG) and
/// returns the landmarks of up to two hands as JSON. Errors are reported in
/// the JSON rather than thrown — nothing crosses the C boundary but a string.
@_cdecl("tafel_detect_hands")
public func tafelDetectHands(image: SRData) -> SRString {
    let request = VNDetectHumanHandPoseRequest()
    request.maximumHandCount = 2
    let handler = VNImageRequestHandler(data: Data(image.toArray()), options: [:])
    do {
        try handler.perform([request])
    } catch {
        return encode(Detection(hands: [], error: error.localizedDescription))
    }
    let hands = (request.results ?? []).map { observation -> Hand in
        var points: [String: Point] = [:]
        for (joint, name) in jointNames {
            guard let point = try? observation.recognizedPoint(joint) else { continue }
            points[name] = Point(
                x: point.location.x,
                y: 1 - point.location.y,
                confidence: Double(point.confidence))
        }
        return Hand(chirality: chiralityName(observation), points: points)
    }
    return encode(Detection(hands: hands, error: nil))
}

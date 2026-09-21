import AppKit

@MainActor
final class GazeStabilizer {
    static let stepDegrees = 22.5
    static let hysteresisDegrees = 7.0
    static let minimumMovement: CGFloat = 2.0

    private(set) var direction = -1
    private var lastTarget = NSPoint.zero

    static func angleDegrees(dx: CGFloat, dy: CGFloat) -> Double {
        (atan2(Double(dx), -Double(dy)) * 180.0 / Double.pi + 360.0)
            .truncatingRemainder(dividingBy: 360.0)
    }

    static func quantize(dx: CGFloat, dy: CGFloat) -> Int {
        Int((angleDegrees(dx: dx, dy: dy) / stepDegrees).rounded()) % 16
    }

    static func unitVector(direction: Int) -> NSPoint {
        let normalized = ((direction % 16) + 16) % 16
        let radians = Double(normalized) * stepDegrees * Double.pi / 180.0
        return NSPoint(x: CGFloat(sin(radians)), y: CGFloat(-cos(radians)))
    }

    func update(dx: CGFloat, dy: CGFloat) -> Int {
        let raw = Self.quantize(dx: dx, dy: dy)
        if direction < 0 {
            direction = raw
            lastTarget = NSPoint(x: dx, y: dy)
            return direction
        }

        let moveX = dx - lastTarget.x
        let moveY = dy - lastTarget.y
        guard hypot(moveX, moveY) >= Self.minimumMovement else {
            return direction
        }
        lastTarget = NSPoint(x: dx, y: dy)

        var delta = Self.angleDegrees(dx: dx, dy: dy) - Double(direction) * Self.stepDegrees
        if delta > 180 { delta -= 360 }
        if delta < -180 { delta += 360 }
        if abs(delta) > (Self.stepDegrees / 2.0) + Self.hysteresisDegrees {
            direction = raw
        }
        return direction
    }

    func reset() {
        direction = -1
        lastTarget = .zero
    }
}

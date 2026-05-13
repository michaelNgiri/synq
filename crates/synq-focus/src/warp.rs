//! Cursor warp calculation — maps cursor position from source to target screen.

use synq_core::{CursorPosition, Edge, ScreenGeometry};

/// Calculate the cursor warp position on the target screen.
///
/// Maps the Y position proportionally between screens and sets X
/// to the opposite edge entry point.
pub fn calculate_warp(
    exit_pos: CursorPosition,
    exit_edge: Edge,
    source_screen: &ScreenGeometry,
    target_screen: &ScreenGeometry,
) -> CursorPosition {
    match exit_edge {
        Edge::Right | Edge::Left => {
            // Proportional Y mapping
            let y_relative = (exit_pos.y - source_screen.y) as f64;
            let y_ratio = y_relative / source_screen.height as f64;
            let target_y = target_screen.y + (y_ratio * target_screen.height as f64) as i32;

            // Enter from the opposite edge
            let target_x = match exit_edge {
                Edge::Right => target_screen.x, // enter from left
                Edge::Left => target_screen.x + target_screen.width as i32 - 1, // enter from right
                _ => unreachable!(),
            };

            CursorPosition {
                x: target_x,
                y: target_y,
            }
        }
        Edge::Top | Edge::Bottom => {
            // Proportional X mapping
            let x_relative = (exit_pos.x - source_screen.x) as f64;
            let x_ratio = x_relative / source_screen.width as f64;
            let target_x = target_screen.x + (x_ratio * target_screen.width as f64) as i32;

            // Enter from the opposite edge
            let target_y = match exit_edge {
                Edge::Top => target_screen.y + target_screen.height as i32 - 1, // enter from bottom
                Edge::Bottom => target_screen.y, // enter from top
                _ => unreachable!(),
            };

            CursorPosition {
                x: target_x,
                y: target_y,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_warp_right_to_left() {
        let source = ScreenGeometry {
            width: 2560,
            height: 1440,
            x: 0,
            y: 0,
        };
        let target = ScreenGeometry {
            width: 1920,
            height: 1080,
            x: 0,
            y: 0,
        };
        let exit = CursorPosition { x: 2559, y: 720 };

        let warp = calculate_warp(exit, Edge::Right, &source, &target);

        assert_eq!(warp.x, 0); // enters from left edge
        assert_eq!(warp.y, 540); // proportional: 720/1440 * 1080 = 540
    }

    #[test]
    fn test_warp_left_to_right() {
        let source = ScreenGeometry {
            width: 1920,
            height: 1080,
            x: 0,
            y: 0,
        };
        let target = ScreenGeometry {
            width: 2560,
            height: 1440,
            x: 0,
            y: 0,
        };
        let exit = CursorPosition { x: 0, y: 540 };

        let warp = calculate_warp(exit, Edge::Left, &source, &target);

        assert_eq!(warp.x, 2559); // enters from right edge
        assert_eq!(warp.y, 720); // proportional: 540/1080 * 1440 = 720
    }
}

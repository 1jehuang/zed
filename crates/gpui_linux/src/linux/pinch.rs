use gpui::{
    Modifiers, PinchEvent, Pixels, PlatformInput, Point, ScrollDelta, ScrollWheelEvent, TouchPhase,
};

/// Convert absolute native pinch scale and logical-center motion into existing
/// GPUI events. Callers dispatch the pinch first so consumers can zoom before
/// panning, including on the first update that leaves the fitted image size.
pub(super) fn pinch_update(
    previous_scale: &mut f32,
    scale: f32,
    position: Point<Pixels>,
    translation: Point<Pixels>,
    modifiers: Modifiers,
) -> (PlatformInput, Option<PlatformInput>) {
    let mut delta = 0.0;
    if scale.is_finite() && scale > 0.0 {
        if previous_scale.is_finite() && *previous_scale > 0.0 {
            let ratio = scale / *previous_scale;
            if ratio.is_finite() && ratio > 0.0 {
                delta = ratio - 1.0;
                *previous_scale = scale;
            }
        } else {
            // Recover a bad baseline without applying an arbitrary zoom jump.
            *previous_scale = scale;
        }
    }

    let pinch = PlatformInput::Pinch(PinchEvent {
        position,
        delta,
        modifiers,
        phase: TouchPhase::Moved,
    });
    // This is direct-manipulation motion, not a wheel axis: retain its sign,
    // pixel units and both axes, regardless of Shift or natural wheel scrolling.
    let x = f32::from(translation.x);
    let y = f32::from(translation.y);
    let scroll = (x.is_finite() && y.is_finite() && (x != 0.0 || y != 0.0)).then(|| {
        PlatformInput::ScrollWheel(ScrollWheelEvent {
            position,
            delta: ScrollDelta::Pixels(translation),
            modifiers,
            touch_phase: TouchPhase::Moved,
        })
    });
    (pinch, scroll)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{point, px};

    fn update(
        previous: &mut f32,
        scale: f32,
        x: f32,
        y: f32,
    ) -> (PinchEvent, Option<ScrollWheelEvent>) {
        let (pinch, scroll) = pinch_update(
            previous,
            scale,
            point(px(100.0), px(80.0)),
            point(px(x), px(y)),
            Modifiers {
                control: true,
                shift: true,
                ..Modifiers::default()
            },
        );
        let PlatformInput::Pinch(pinch) = pinch else {
            panic!("pinch must be first")
        };
        let scroll = scroll.map(|input| {
            let PlatformInput::ScrollWheel(scroll) = input else {
                panic!("translation must be scroll")
            };
            scroll
        });
        (pinch, scroll)
    }

    #[test]
    fn cumulative_scales_compose_and_return_to_start() {
        let mut previous = 1.0;
        let mut zoom = 1.0;
        for scale in [1.2, 1.5, 2.0, 1.0, 0.5, 1.0] {
            let (pinch, scroll) = update(&mut previous, scale, 0.0, 0.0);
            zoom *= 1.0 + pinch.delta;
            assert!((zoom - scale).abs() < 0.00001);
            assert_eq!(previous, scale);
            assert!(scroll.is_none());
        }
    }

    #[test]
    fn simultaneous_pan_keeps_position_modifiers_sign_and_pixel_units() {
        let mut previous = 1.0;
        let (pinch, scroll) = update(&mut previous, 1.5, -0.25, 4.5);
        assert_eq!(pinch.delta, 0.5);
        assert_eq!(pinch.phase, TouchPhase::Moved);
        let scroll = scroll.unwrap();
        assert_eq!(scroll.position, pinch.position);
        assert_eq!(scroll.modifiers, pinch.modifiers);
        assert!(scroll.modifiers.control && scroll.modifiers.shift);
        assert_eq!(scroll.touch_phase, TouchPhase::Moved);
        let ScrollDelta::Pixels(delta) = scroll.delta else {
            panic!("must not emit lines")
        };
        assert_eq!(delta, point(px(-0.25), px(4.5)));
    }

    #[test]
    fn unchanged_scale_still_pans_and_zero_translation_does_not_scroll() {
        let mut previous = 2.0;
        let (pinch, scroll) = update(&mut previous, 2.0, 3.0, 0.0);
        assert_eq!(pinch.delta, 0.0);
        assert!(scroll.is_some());
        assert!(update(&mut previous, 2.0, 0.0, -0.0).1.is_none());
    }

    #[test]
    fn invalid_scales_do_not_poison_baseline_or_discard_pan() {
        let mut previous = 1.0;
        for invalid in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            let (pinch, scroll) = update(&mut previous, invalid, 1.0, 2.0);
            assert_eq!(pinch.delta, 0.0);
            assert_eq!(previous, 1.0);
            assert!(scroll.is_some());
        }
        assert_eq!(update(&mut previous, 1.5, 0.0, 0.0).0.delta, 0.5);
        previous = f32::NAN;
        assert_eq!(update(&mut previous, 2.0, 0.0, 0.0).0.delta, 0.0);
        assert_eq!(previous, 2.0);
    }

    #[test]
    fn invalid_translation_does_not_discard_zoom() {
        let mut previous = 1.0;
        let (pinch, scroll) = update(&mut previous, 2.0, f32::NAN, 1.0);
        assert_eq!(pinch.delta, 1.0);
        assert!(scroll.is_none());
        assert!(update(&mut previous, 2.0, 1.0, f32::INFINITY).1.is_none());
    }
}

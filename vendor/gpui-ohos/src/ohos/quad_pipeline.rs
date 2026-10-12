use gpui::{BorderStyle, Quad};

/// Compile away unused fragment work without changing the general shader's
/// clipping, blending, rounded corners, gradients, or dashed borders.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum QuadPipeline {
    General,
    Solid,
    Rect,
}

impl QuadPipeline {
    pub(crate) fn for_quad(quad: &Quad) -> Self {
        if quad.background.as_solid().is_none() || quad.border_style != BorderStyle::Solid {
            return Self::General;
        }
        let radii = &quad.corner_radii;
        let borders = &quad.border_widths;
        if [
            radii.top_left.0,
            radii.top_right.0,
            radii.bottom_right.0,
            radii.bottom_left.0,
            borders.top.0,
            borders.right.0,
            borders.bottom.0,
            borders.left.0,
        ]
        .into_iter()
        .all(|value| value == 0.0)
        {
            Self::Rect
        } else {
            Self::Solid
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{ScaledPixels, black, linear_color_stop, linear_gradient, white};

    fn solid_quad() -> Quad {
        Quad {
            order: 0,
            border_style: BorderStyle::Solid,
            bounds: Default::default(),
            content_mask: Default::default(),
            background: black().into(),
            border_color: black(),
            corner_radii: Default::default(),
            border_widths: Default::default(),
        }
    }

    #[test]
    fn gradients_and_dashes_keep_the_general_shader() {
        let mut quad = solid_quad();
        quad.background = linear_gradient(
            90.0,
            linear_color_stop(black(), 0.0),
            linear_color_stop(white(), 1.0),
        );
        assert_eq!(QuadPipeline::for_quad(&quad), QuadPipeline::General);
        quad.background = black().into();
        quad.border_style = BorderStyle::Dashed;
        assert_eq!(QuadPipeline::for_quad(&quad), QuadPipeline::General);
    }

    #[test]
    fn rect_path_never_bypasses_any_corner_or_border() {
        let mut quad = solid_quad();
        assert_eq!(QuadPipeline::for_quad(&quad), QuadPipeline::Rect);
        for index in 0..8 {
            let parts = [
                &mut quad.corner_radii.top_left,
                &mut quad.corner_radii.top_right,
                &mut quad.corner_radii.bottom_right,
                &mut quad.corner_radii.bottom_left,
                &mut quad.border_widths.top,
                &mut quad.border_widths.right,
                &mut quad.border_widths.bottom,
                &mut quad.border_widths.left,
            ];
            *parts[index] = ScaledPixels(0.5);
            assert_eq!(QuadPipeline::for_quad(&quad), QuadPipeline::Solid);
            quad = solid_quad();
        }
    }
}

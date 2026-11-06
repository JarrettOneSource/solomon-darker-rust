//! Drawable size and window ownership, independent of the world/UI painters.

#[derive(Clone)]
pub struct Surface {
    #[cfg(target_arch = "wasm32")]
    canvas: web_sys::HtmlCanvasElement,
    #[cfg(not(target_arch = "wasm32"))]
    window: std::sync::Arc<winit::window::Window>,
}

impl Surface {
    #[cfg(target_arch = "wasm32")]
    pub fn from_canvas(canvas: web_sys::HtmlCanvasElement) -> Self {
        Self { canvas }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_window(window: std::sync::Arc<winit::window::Window>) -> Self {
        Self { window }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn canvas(&self) -> &web_sys::HtmlCanvasElement {
        &self.canvas
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn window(&self) -> &std::sync::Arc<winit::window::Window> {
        &self.window
    }

    pub fn visible_height(&self) -> f64 {
        #[cfg(target_arch = "wasm32")]
        if let Some(viewport) = self
            .canvas
            .owner_document()
            .and_then(|document| document.default_view())
            .and_then(|window| window.visual_viewport())
        {
            return viewport.height();
        }
        f64::from(self.logical_height())
    }

    pub fn logical_width(&self) -> i32 {
        self.logical_size()[0]
    }

    pub fn logical_height(&self) -> i32 {
        self.logical_size()[1]
    }

    pub fn logical_size(&self) -> [i32; 2] {
        #[cfg(target_arch = "wasm32")]
        {
            [self.canvas.client_width(), self.canvas.client_height()]
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let size = self.window.inner_size().to_logical::<f64>(self.scale_factor());
            [size.width as i32, size.height as i32]
        }
    }

    pub fn physical_size(&self) -> [i32; 2] {
        #[cfg(target_arch = "wasm32")]
        {
            [self.canvas.width() as i32, self.canvas.height() as i32]
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let size = self.window.inner_size();
            [size.width as i32, size.height as i32]
        }
    }

    pub fn scale_factor(&self) -> f64 {
        #[cfg(target_arch = "wasm32")]
        {
            self.canvas
                .owner_document()
                .and_then(|document| document.default_view())
                .map_or(1.0, |window| window.device_pixel_ratio())
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.window.scale_factor()
        }
    }

    pub fn origin(&self) -> [f64; 2] {
        #[cfg(target_arch = "wasm32")]
        {
            let bounds = self.canvas.get_bounding_client_rect();
            [bounds.left(), bounds.top()]
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            [0.0, 0.0]
        }
    }

    pub fn point_fraction(&self, point: solomon_sim::Vec2) -> Option<solomon_sim::Vec2> {
        #[cfg(target_arch = "wasm32")]
        let [width, height] = {
            let bounds = self.canvas.get_bounding_client_rect();
            [bounds.width(), bounds.height()]
        };
        #[cfg(not(target_arch = "wasm32"))]
        let [width, height] = {
            let size = self.window.inner_size().to_logical::<f64>(self.scale_factor());
            [size.width, size.height]
        };
        let [left, top] = self.origin();
        (width > 0.0 && height > 0.0)
            .then(|| solomon_sim::Vec2::new((point.x - left) / width, (point.y - top) / height))
    }

    /// Browser canvases own their drawing-buffer size. A native GL surface is
    /// resized by its event-loop owner before the shared client draws a frame.
    pub fn fit_buffer(&self) -> (i32, i32) {
        #[cfg(target_arch = "wasm32")]
        {
            let [width, height] = self
                .logical_size()
                .map(|value| (f64::from(value) * self.scale_factor()).round().max(1.0) as u32);
            if self.canvas.width() != width {
                self.canvas.set_width(width);
            }
            if self.canvas.height() != height {
                self.canvas.set_height(height);
            }
        }
        let [width, height] = self.physical_size();
        (width, height)
    }

    pub fn coarse_without_hover(&self) -> Result<bool, String> {
        #[cfg(target_arch = "wasm32")]
        {
            self.canvas
                .owner_document()
                .and_then(|document| document.default_view())
                .ok_or("The game has no window")?
                .match_media("(hover:none) and (pointer:coarse)")
                .map_err(|_| "Could not read the pointer type".to_owned())
                .map(|query| query.is_some_and(|query| query.matches()))
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            Ok(super::coarse_pointer())
        }
    }

    pub fn portrait_touch(&self) -> Result<bool, String> {
        #[cfg(target_arch = "wasm32")]
        {
            self.canvas
                .owner_document()
                .and_then(|document| document.default_view())
                .ok_or("The game has no window")?
                .match_media("(orientation:portrait) and (hover:none) and (pointer:coarse)")
                .map_err(|_| "Could not read the display orientation".to_owned())
                .map(|query| query.is_some_and(|query| query.matches()))
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let [width, height] = self.logical_size();
            Ok(super::coarse_pointer() && height > width)
        }
    }

    pub fn touch_capable(&self) -> bool {
        #[cfg(target_arch = "wasm32")]
        {
            self.canvas
                .owner_document()
                .and_then(|document| document.default_view())
                .is_some_and(|window| window.navigator().max_touch_points() > 0)
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            super::coarse_pointer()
        }
    }
}

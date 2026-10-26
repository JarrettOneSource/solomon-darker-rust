//! One OS window and current OpenGL context, using winit and glutin.

use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::Arc;

use glutin::config::{ConfigTemplateBuilder, GlConfig};
use glutin::context::{
    ContextApi, ContextAttributesBuilder, GlProfile, PossiblyCurrentContext, Version,
};
use glutin::display::{GetGlDisplay, GlDisplay};
use glutin::prelude::*;
use glutin::surface::{Surface, SurfaceAttributesBuilder, SwapInterval, WindowSurface};
use glutin_winit::DisplayBuilder;
use raw_window_handle::HasWindowHandle;
use solomon_graphics::{GraphicsContext, Renderer};
use winit::dpi::LogicalSize;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

pub struct NativeWindow {
    gl: GraphicsContext,
    surface: Surface<WindowSurface>,
    context: PossiblyCurrentContext,
    // Driver objects retire before the OS destroys the window they reference.
    pub window: Arc<Window>,
}

impl NativeWindow {
    pub fn open(event_loop: &ActiveEventLoop, title: &str, size: [u32; 2]) -> Result<Self, String> {
        let attributes = Window::default_attributes()
            .with_title(title)
            // AccessKit must attach before the first show on macOS.
            .with_visible(false)
            .with_inner_size(LogicalSize::new(size[0], size[1]));
        let template = ConfigTemplateBuilder::new()
            .with_alpha_size(8)
            .with_depth_size(0)
            .with_stencil_size(0)
            .with_transparency(false);
        let (window, config) = DisplayBuilder::new()
            .with_window_attributes(Some(attributes))
            .build(event_loop, template, |configs| {
                configs
                    .min_by_key(|config| config.num_samples())
                    .expect("glutin supplied a nonempty compatible configuration set")
            })
            .map_err(|error| format!("Open native window: {error}"))?;
        let window = Arc::new(window.ok_or("The platform did not create the game window")?);
        let handle = window
            .window_handle()
            .map_err(|error| format!("Native window handle: {error}"))?
            .as_raw();
        let attributes = ContextAttributesBuilder::new()
            .with_profile(GlProfile::Core)
            .with_context_api(ContextApi::OpenGl(Some(Version::new(3, 3))))
            .build(Some(handle));
        let display = config.display();
        // SAFETY: the live winit window owns this handle and outlives its context/surface.
        let context = unsafe { display.create_context(&config, &attributes) }
            .map_err(|error| format!("Create native graphics context: {error}"))?;
        let size = window.inner_size();
        let attributes = SurfaceAttributesBuilder::<WindowSurface>::new()
            .with_srgb(Some(false))
            .build(handle, nonzero(size.width), nonzero(size.height));
        // SAFETY: the surface uses the same live window and display as its context.
        let surface = unsafe { display.create_window_surface(&config, &attributes) }
            .map_err(|error| format!("Create native graphics surface: {error}"))?;
        let context = context
            .make_current(&surface)
            .map_err(|error| format!("Activate native graphics: {error}"))?;
        // SAFETY: glutin has made the context current and resolves its own driver symbols.
        let gl = Rc::new(unsafe {
            glow::Context::from_loader_function_cstr(|name| display.get_proc_address(name))
        });
        surface
            .set_swap_interval(&context, SwapInterval::Wait(nonzero(1)))
            .map_err(|error| format!("Set native display synchronization: {error}"))?;
        Ok(Self { window, gl, surface, context })
    }

    /// # Safety
    /// Keep this window/context alive and current until the renderer and its caches drop.
    pub unsafe fn renderer(&self) -> Result<Renderer, String> {
        self.make_current()?;
        // SAFETY: NativeWindow owns this current context until the application's painter
        // and caches have been dropped. The window loop renders on this same thread.
        unsafe { Renderer::for_native(self.gl.clone()) }
    }

    pub fn graphics_context(&self) -> GraphicsContext {
        self.gl.clone()
    }

    pub fn make_current(&self) -> Result<(), String> {
        self.context
            .make_current(&self.surface)
            .map_err(|error| format!("Activate native graphics: {error}"))
    }

    pub fn resize(&self, width: u32, height: u32) {
        self.surface.resize(&self.context, nonzero(width), nonzero(height));
    }

    pub fn present(&self) -> Result<(), String> {
        self.surface
            .swap_buffers(&self.context)
            .map_err(|error| format!("Present native frame: {error}"))
    }
}

fn nonzero(value: u32) -> NonZeroU32 {
    NonZeroU32::new(value.max(1)).expect("clamped positive window extent")
}

use glyphon::{
    Attrs, Buffer, Color, Family, FontSystem, Metrics, Resolution, Shaping, SwashCache, TextArea,
    TextAtlas, TextBounds, TextRenderer as GlyphonTextRenderer,
};
use wgpu::{Device, MultisampleState, Queue, RenderPass, SurfaceConfiguration};
use anyhow::Result;

pub struct TextRenderer {
    font_system: FontSystem,
    swash_cache: SwashCache,
    viewport: ValidationViewport,
    atlas: TextAtlas,
    text_renderer: GlyphonTextRenderer,
    pub buffer: Buffer,
}

struct ValidationViewport {
    width: u32,
    height: u32,
}

impl TextRenderer {
    pub fn new(
        device: &Device,
        queue: &Queue,
        config: &SurfaceConfiguration,
    ) -> Result<Self> {

        let mut font_system = FontSystem::new();
        let swash_cache = SwashCache::new();
        let viewport = ValidationViewport {
            width: config.width,
            height: config.height,
        };
        let mut atlas = TextAtlas::new(device, queue, config.format);
        let text_renderer =
            GlyphonTextRenderer::new(&mut atlas, device, MultisampleState::default(), None);
        let mut buffer = Buffer::new(&mut font_system, Metrics::new(20.0, 25.0));

        buffer.set_size(&mut font_system, config.width as f32, config.height as f32);
        buffer.set_text(&mut font_system, "", Attrs::new().family(Family::SansSerif), Shaping::Advanced);
        buffer.shape_until_scroll(&mut font_system);

        Ok(Self {
            font_system,
            swash_cache,
            viewport,
            atlas,
            text_renderer,
            buffer,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.viewport.width = width;
        self.viewport.height = height;
        self.buffer.set_size(&mut self.font_system, width as f32, height as f32);
        self.buffer.shape_until_scroll(&mut self.font_system);
    }

    pub fn set_text(&mut self, text: &str) {
        self.buffer.set_text(&mut self.font_system, text, Attrs::new().family(Family::SansSerif).color(Color::rgb(255, 255, 255)), Shaping::Advanced);
        self.buffer.shape_until_scroll(&mut self.font_system);
    }

    pub fn render<'a>(
        &'a mut self,
        device: &Device,
        queue: &Queue,
        pass: &mut RenderPass<'a>,
    ) -> Result<()> {
        self.text_renderer.prepare(
            device,
            queue,
            &mut self.font_system,
            &mut self.atlas,
            Resolution {
                width: self.viewport.width,
                height: self.viewport.height,
            },
            [TextArea {
                buffer: &self.buffer,
                left: 10.0,
                top: 10.0,
                scale: 1.0,
                bounds: TextBounds {
                    left: 0,
                    top: 0,
                    right: self.viewport.width as i32,
                    bottom: self.viewport.height as i32,
                },
                default_color: Color::rgb(255, 255, 255),
            }],
            &mut self.swash_cache,
        )?;

        self.text_renderer.render(&self.atlas, pass).map_err(|e| anyhow::anyhow!(e))?;
        Ok(())
    }
}

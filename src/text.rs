use anyhow::Result;
use wgpu::{Device, Queue, RenderPass, SurfaceConfiguration};

pub struct TextRenderer {}

impl TextRenderer {
    pub fn new(
        _device: &Device,
        _queue: &Queue,
        _config: &SurfaceConfiguration,
        _font_family: Option<&str>,
    ) -> Result<Self> {
        Ok(Self {})
    }

    pub fn set_style(&mut self, _font_size: f32, _color: [u8; 4]) {}
    pub fn resize(&mut self, _queue: &Queue, _width: u32, _height: u32) {}
    pub fn set_text(&mut self, _text: &str) {}
    pub fn set_osd_text(&mut self, _text: &str) {}
    pub fn set_info_text(&mut self, _text: &str) {}
    pub fn toggle_info_overlay(&mut self) -> bool {
        false
    }
    pub fn info_overlay_visible(&self) -> bool {
        false
    }
    pub fn render<'a>(
        &'a mut self,
        _device: &Device,
        _queue: &Queue,
        _pass: &mut RenderPass<'a>,
    ) -> Result<()> {
        Ok(())
    }
}

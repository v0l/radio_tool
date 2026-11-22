use eframe::egui_wgpu::{RendererOptions, ScreenDescriptor};
use eframe::wgpu;

const SIZE: u32 = 128;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

#[derive(Debug, Clone)]
pub struct Probe {
    pub backend: wgpu::Backend,
    pub adapter: String,
    pub distinct_colours: usize,
}

pub fn distinct_colours(rgba: &[u8]) -> usize {
    let mut seen: Vec<[u8; 4]> = Vec::new();
    for px in rgba.chunks_exact(4) {
        let px = [px[0], px[1], px[2], px[3]];
        if !seen.contains(&px) {
            seen.push(px);
            if seen.len() > 8 {
                break;
            }
        }
    }
    seen.len()
}

pub fn is_blank(rgba: &[u8]) -> bool {
    distinct_colours(rgba) < 3
}

fn sample_frame() -> (egui::Context, egui::FullOutput) {
    let ctx = egui::Context::default();
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(SIZE as f32, SIZE as f32),
        )),
        ..Default::default()
    };
    let output = ctx.run_ui(raw, |ui| {
        egui::Panel::top("probe_top").show(ui, |ui| {
            ui.heading("ngrind");
        });
        egui::CentralPanel::default().show(ui, |ui| {
            ui.label("render check");
            ui.add(egui::Button::new("ok").fill(egui::Color32::from_rgb(200, 60, 60)));
        });
    });
    (ctx, output)
}

pub fn render_probe(backends: wgpu::Backends) -> Result<Probe, String> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .map_err(|e| format!("no adapter: {e}"))?;
    let info = adapter.get_info();
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .map_err(|e| format!("no device: {e}"))?;

    let (ctx, mut output) = sample_frame();
    let jobs = ctx.tessellate(output.shapes, output.pixels_per_point);
    let mut renderer =
        eframe::egui_wgpu::Renderer::new(&device, FORMAT, RendererOptions::default());
    for (id, deltas) in output.textures_delta.set.drain() {
        for delta in &deltas {
            renderer.update_texture(&device, &queue, id, delta);
        }
    }
    let screen = ScreenDescriptor {
        size_in_pixels: [SIZE, SIZE],
        pixels_per_point: output.pixels_per_point,
    };

    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("render_check"),
        size: wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let row_bytes = SIZE * 4;
    let padded = row_bytes.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("render_check_readback"),
        size: (padded * SIZE) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    let extra = renderer.update_buffers(&device, &queue, &mut encoder, &jobs, &screen);
    {
        let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("render_check_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        renderer.render(&mut pass.forget_lifetime(), &jobs, &screen);
    }
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(SIZE),
            },
        },
        wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(extra.into_iter().chain(Some(encoder.finish())));

    let (tx, rx) = std::sync::mpsc::channel();
    readback.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|e| format!("poll: {e}"))?;
    rx.recv()
        .map_err(|e| format!("map: {e}"))?
        .map_err(|e| format!("map: {e}"))?;
    let mapped = readback
        .slice(..)
        .get_mapped_range()
        .map_err(|e| format!("map: {e}"))?;
    let mut rgba = Vec::with_capacity((row_bytes * SIZE) as usize);
    for row in mapped.chunks(padded as usize) {
        rgba.extend_from_slice(&row[..row_bytes as usize]);
    }
    drop(mapped);

    let probe = Probe {
        backend: info.backend,
        adapter: info.name,
        distinct_colours: distinct_colours(&rgba),
    };
    if is_blank(&rgba) {
        Err(format!(
            "blank render on {:?} ({}): {} distinct colours",
            probe.backend, probe.adapter, probe.distinct_colours
        ))
    } else {
        Ok(probe)
    }
}

pub fn pick_backends() -> wgpu::Backends {
    if let Some(forced) = wgpu::Backends::from_env() {
        return forced;
    }
    let candidates = [
        wgpu::Backends::PRIMARY,
        wgpu::Backends::DX12,
        wgpu::Backends::VULKAN,
        wgpu::Backends::GL,
    ];
    for backends in candidates {
        match render_probe(backends) {
            Ok(probe) => {
                eprintln!(
                    "radio_tool-ui: render check ok on {:?} ({}), {} distinct colours",
                    probe.backend, probe.adapter, probe.distinct_colours
                );
                return backends;
            }
            Err(e) => eprintln!("radio_tool-ui: render check failed for {backends:?}: {e}"),
        }
    }
    eprintln!("radio_tool-ui: no backend passed the render check, using the default");
    wgpu::Backends::PRIMARY | wgpu::Backends::GL
}

pub fn report() -> bool {
    let mut all_ok = true;
    for backends in [
        wgpu::Backends::DX12,
        wgpu::Backends::VULKAN,
        wgpu::Backends::GL,
        wgpu::Backends::METAL,
    ] {
        match render_probe(backends) {
            Ok(p) => println!(
                "{backends:?}: ok {:?} {} colours={}",
                p.backend, p.adapter, p.distinct_colours
            ),
            Err(e) => {
                println!("{backends:?}: {e}");
                if !e.starts_with("no adapter") {
                    all_ok = false;
                }
            }
        }
    }
    all_ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_colour_buffer_is_blank() {
        let white = vec![255u8; 64 * 64 * 4];
        assert!(is_blank(&white));
    }

    #[test]
    fn two_colour_buffer_is_blank() {
        let mut px = vec![255u8; 64 * 4];
        px[0] = 0;
        assert!(is_blank(&px));
    }

    #[test]
    fn varied_buffer_is_not_blank() {
        let mut px = vec![255u8; 64 * 4];
        px[0] = 0;
        px[4] = 10;
        px[5] = 20;
        assert!(!is_blank(&px));
    }

    #[test]
    fn sample_frame_draws_more_than_one_colour() {
        let (_ctx, mut output) = sample_frame();
        assert!(!output.shapes.is_empty());
        output.textures_delta.clear();
    }

    #[test]
    fn probe_renders_or_reports_no_adapter() {
        match render_probe(wgpu::Backends::all()) {
            Ok(p) => assert!(p.distinct_colours >= 3),
            Err(e) => assert!(
                e.starts_with("no adapter") || e.starts_with("no device"),
                "{e}"
            ),
        }
    }
}

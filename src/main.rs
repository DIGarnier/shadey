// TODO:
// 1. GUI load file DONE!
// 2. Logic compile and run shaders DONE!
// 3. Parse shader to find uniforms DONE!
//      3.1. Compute memory layout of GUI struct DONE!
// 4. Autocreate UI to modify uniform DONE-ish
// 5. Watch shader file for change and autoreload DONE !
// 6. Parse comment options DONE-ish
//      6.1. Add texture loading DONE!
//      6.2. ...
// 7. Build up widget libraries
// 8. Gracefully handle bad app states

mod app;
mod parser;
mod shader;
mod texture;
mod ui;
mod wgsl;

use app::App;
use winit::{event::Event, event_loop::{ControlFlow, EventLoopBuilder}};

fn main() {
    let event_loop = EventLoopBuilder::with_user_event().build();
    let mut app = pollster::block_on(App::new(&event_loop));

    let event_loop_proxy = event_loop.create_proxy();
    event_loop.run(move |event, _, control_flow| {
        app.platform_handle_event(&event);

        match event {
            Event::WindowEvent {
                event: ref window_event,
                window_id,
            } => app.handle_window_event(window_id, control_flow, window_event),
            Event::RedrawRequested(_) => {
                app.update();
                match app.render() {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost) => app.resize((app.config.height, app.config.width).into()),
                    Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    Err(e) => eprintln!("{:?}", e),
                }
            }
            Event::MainEventsCleared => app.request_redraw(),
            Event::UserEvent(user_event) => app.handle_user_event(user_event, &event_loop_proxy),
            _ => ()
        }
    });
}


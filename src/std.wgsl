struct StdUniform {
    window_size: vec2<u32>;
    mouse_pos: vec2<u32>;
    time: f32;
};

[[group(0), binding(0)]]
var<uniform> _std_uniform: StdUniform;

fn norm_coords(texcoords: vec2<f32>) -> vec2<f32> {
    let ratio = f32(_std_uniform.window_size.x)/f32(_std_uniform.window_size.y);
    return vec2<f32>(texcoords.x*ratio, texcoords.y);
}

fn window_size() -> vec2<u32> {
    return _std_uniform.window_size;
}

fn mouse_pos() -> vec2<f32> {
    var mouse_pos = vec2<f32>(_std_uniform.mouse_pos.xy)/vec2<f32>(_std_uniform.window_size.xy);
    mouse_pos.y = 1. - mouse_pos.y;
    return mouse_pos;
}

fn time() -> f32 {
    return _std_uniform.time;
}

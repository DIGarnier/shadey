// pain points
// - graph widget x-y
// toggle mouse_pos

struct GuiControlled {
    timespan: u32;
    scale: f32;
    x: f32;
    y: f32;
};

[[group(0), binding(1)]]
var<uniform> gui: GuiControlled;


//Generate 2 somewhat random numbers
fn random2(p: vec2<f32>, i: f32) -> vec2<f32> {
    return fract(sin(vec2<f32>(dot(p,vec2<f32>(15.2,i)),dot(p,vec2<f32>(26.0,18.0))))*4785.3);
}

//Generate pseudo random numbers, following "The book of Shaders"
fn random(st: vec2<f32>) -> f32 {
    return fract(sin(dot(st.xy,vec2<f32>(12.9898,78.233)))*43758.5453123);
}

fn circle(p: vec2<f32>, r: f32) -> f32
{
    return length(p) - r;
}

// Fragment shader
[[stage(fragment)]]
fn fs_main(vo: VertexOutput) -> [[location(0)]] vec4<f32> {
    let spos = screen_coords(vo, toggle_mouse_pos());

    var d = circle(spos, 0.05);
    var col = vec3<f32>(0.0 + sign(d) * 1.0);

    return vec4<f32>(col,1.);
}
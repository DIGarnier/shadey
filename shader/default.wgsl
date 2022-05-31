struct GuiControlled {
    speed: f32; // a
    expo: f32; // wow
    colorf32: vec3<f32>; // b
    coloru32: vec3<u32>; // b
    circle_r: f32; // v
};

// Shadey
// texture(path=texture/tex.jpg, alias=davey)


fn to_ring(d:f32, r:f32) -> f32
{
    return abs(d) - r;
}

fn circle(p: vec2<f32>, r: f32) -> f32
{
    return length(p) - r;
}

fn box(p: vec2<f32>, b: vec2<f32> ) -> f32
{
    let d = abs(p)-b;
    return length(max(d,vec2<f32>(0.0))) + min(max(d.x,d.y),0.0);
}


// Fragment shader
[[stage(fragment)]]
fn fs_main(vo: VertexOutput) -> [[location(0)]] vec4<f32> {
    let mouse_pos = toggle_mouse_pos();
    let t = time() * (speed() + 1.0);
    var shape_pos = vec2<f32>(sin(t/1.57), cos(t))/2.5 + 0.5;
    var p = screen_coords(vo, mouse_pos);

    var d = circle(p, circle_r());
    var d1 = box(p, vec2<f32>(0.3, 0.1));
    d = to_ring(d+d1, 0.05);
    var col = vec3<f32>(1.0) - sign(d)*vec3<f32>(shape_pos,0.7);
    col = mix(col, colorf32(), 1.0-smoothStep(0.0,expo(),abs(d)));

    return vec4<f32>(col,1.);
}
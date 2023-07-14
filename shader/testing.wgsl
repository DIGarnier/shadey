// pain points
// - graph widget x-y

struct GuiControlled {
    timespan: f32, // range(min=1,max=50)
    scale: u32, // range(min=1,max=20)
    circle_scale: f32, // range(min=0.01,max=1.0)
    x: f32,
    y: f32,
    color: vec3<f32>,
};


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

fn swirl(p: vec2<f32>) -> f32 {
    let r = length(p);
    let a = atan2(p.y,p.x);
    return r - 1.0 + sin(f32(scale())*a+2.0*r*r)/2.0;
}

fn rot2d(pos: vec2<f32>, o: f32) -> vec2<f32> {
    let coso = cos(o);
    let sino = sin(o);
    return vec2<f32>(pos.x*coso-pos.y*sino, pos.x*sino+pos.y*coso);
}


@fragment
fn fs_main(vo: VertexOutput) -> @location(0) vec4<f32> {
    let t = time()*timespan();

    var spos = screen_coords(vo, vec2<f32>(0.5));

    spos = rot2d(spos, t) * 5.0;

    spos = rot2d(spos, sin(length(spos)*10.*x()+t*2.));
    var cir = circle(spos, circle_scale());
    var v = vec3<f32>(abs(swirl(spos)*cir));
    let eps = 0.03;
    var col = smoothstep(color(), vec3<f32>(0.0), v);

    return vec4<f32>(col,1.);
}
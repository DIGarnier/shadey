struct GuiControlled {
    x: f32; // range(min=-2,max=2)
    y: f32; // range(min=-2,max=2)
    z: f32; // range(min=-2,max=2)
    noise: f32; // range(min=1,max=10)
    rep: u32; // range(min=0,max=100)
    color: vec3<f32>;
};


fn circle(p: vec2<f32>, r: f32) -> f32
{
    return length(p) - r;
}

fn sphere(p: vec3<f32>, r: f32) -> f32
{
    return length(p) - r;
}

fn camera(ro: vec3<f32>, ta: vec3<f32>, cr: f32) -> mat3x3<f32>
{
	let cw = normalize(ta-ro);
	let cp = vec3<f32>(sin(cr), cos(cr), 0.0);
	let cu = normalize(cross(cw, cp));
	let cv = normalize(cross(cu, cw));
    return mat3x3<f32>(cu, cv, cw);
}

// https://iquilezles.org/articles/boxfunctions
fn iBox(ro: vec3<f32>, rd: vec3<f32>, rad: vec3<f32>) -> vec2<f32>
{
    let m = 1.0/rd;
    let n = m*ro;
    let k = abs(m)*rad;
    let t1 = -n - k;
    let t2 = -n + k;
	return vec2<f32>(max(max(t1.x, t1.y), t1.z),
	                 min(min(t2.x, t2.y), t2.z));
}


fn repeat_inf_sphere(p: vec3<f32>, c: vec3<f32>) -> f32
{
    var q = ((p+(0.5*c)) % c) - (0.5*c);
    return sphere(q, 0.2);
}

fn repeat_lim(p: vec3<f32>, c: f32, l: f32) -> f32
{
    var q = p-c*clamp(round(p/c),-vec3<f32>(l),vec3<f32>(l));
    return sphere(q, 0.2); 
}

fn permute4(x: vec4<f32>) -> vec4<f32> { return ((x * 34. + 1.) * x) % vec4<f32>(289.); }
fn fade2(t: vec2<f32>) -> vec2<f32> { return t * t * t * (t * (t * 6. - 15.) + 10.); }

fn perlinNoise2(P: vec2<f32>) -> f32 {
  var Pi: vec4<f32> = floor(P.xyxy) + vec4<f32>(0., 0., 1., 1.);
  let Pf = fract(P.xyxy) - vec4<f32>(0., 0., 1., 1.);
  Pi = Pi % vec4<f32>(289.); // To avoid truncation effects in permutation
  let ix = Pi.xzxz;
  let iy = Pi.yyww;
  let fx = Pf.xzxz;
  let fy = Pf.yyww;
  let i = permute4(permute4(ix) + iy);
  var gx: vec4<f32> = 2. * fract(i * 0.0243902439) - 1.; // 1/41 = 0.024...
  let gy = abs(gx) - 0.5;
  let tx = floor(gx + 0.5);
  gx = gx - tx;
  var g00: vec2<f32> = vec2<f32>(gx.x, gy.x);
  var g10: vec2<f32> = vec2<f32>(gx.y, gy.y);
  var g01: vec2<f32> = vec2<f32>(gx.z, gy.z);
  var g11: vec2<f32> = vec2<f32>(gx.w, gy.w);
  let norm = 1.79284291400159 - 0.85373472095314 *
      vec4<f32>(dot(g00, g00), dot(g01, g01), dot(g10, g10), dot(g11, g11));
  g00 = g00 * norm.x;
  g01 = g01 * norm.y;
  g10 = g10 * norm.z;
  g11 = g11 * norm.w;
  let n00 = dot(g00, vec2<f32>(fx.x, fy.x));
  let n10 = dot(g10, vec2<f32>(fx.y, fy.y));
  let n01 = dot(g01, vec2<f32>(fx.z, fy.z));
  let n11 = dot(g11, vec2<f32>(fx.w, fy.w));
  let fade_xy = fade2(Pf.xy);
  let n_x = mix(vec2<f32>(n00, n01), vec2<f32>(n10, n11), vec2<f32>(fade_xy.x));
  let n_xy = mix(n_x.x, n_x.y, fade_xy.y);
  return 2.3 * n_xy;
}

fn sdfBox(p: vec3<f32>, size: vec3<f32>) -> f32 {
  let d = abs(p) - size;
  return length(max(d, vec3<f32>(0.0))) + min(max(d.x, max(d.y, d.z)), 0.0);
}

fn sdfCylinder(p: vec3<f32>, size: vec2<f32>) -> f32 {
  let d = vec2<f32>(length(p.xz), abs(p.y)) - size;
  return min(max(d.x, d.y), 0.0) + length(max(d, vec2<f32>(0.0)));
}

fn rotate3D(p: vec3<f32>, axis: vec3<f32>, angle: f32) -> vec3<f32> {
  let c = cos(angle);
  let s = sin(angle);
  let t = 1.0 - c;
  
  let axis = normalize(axis);
  let x = axis.x;
  let y = axis.y;
  let z = axis.z;
  
  let rot = mat3x3<f32>(
    vec3<f32>(t * x * x + c, t * x * y - s * z, t * x * z + s * y),
    vec3<f32>(t * x * y + s * z, t * y * y + c, t * y * z - s * x),
    vec3<f32>(t * x * z - s * y, t * y * z + s * x, t * z * z + c)
  );
  
  return rot * p;
}


fn map(pos: vec3<f32>) -> vec2<f32>
{
    var npos = pos;
    npos.y = npos.y / 2.0;
    let k = 40.0*npos.y;
    npos.x = npos.x + sin(k+10.0*time())/40.0;
    npos.z = npos.z + cos(k+10.0*time())/40.0;
 
    npos = npos - vec3<f32>(0.0, 0.4, 0.0); 
    npos = npos + perlinNoise2(pos.xz*noise() + time())/8.0;
    var lol = sphere(npos , 0.4);
    // lol = repeat_inf_sphere(npos, vec3<f32>(1.0));

    // lol = lol + sin(pos.x*5.0 + time())*sin(pos.y*10.0 + time())*sin(pos.z*15.0 + time())/ 10.0;

    return vec2<f32>(lol, 26.9);
}


fn raycast(ro: vec3<f32>, rd: vec3<f32>) -> vec2<f32>
{
    var res = vec2<f32>(-1.0);
    var tmin = 1.0;
    var tmax = 200.0;

    // raytrace floor plane
    let tp1 = (-ro.y)/rd.y;
    if (tp1 > 0.0)
    {
        tmax = min(tmax, tp1);
        res = vec2<f32>(tp1, 1.0);
    }
    
    // raymarch primitives   

    var t = tmin;
    for (var i=0; i<70 && t<tmax; i = i+1)
    {
        var h = map(ro+rd*t);
        if (abs(h.x)<(0.0001*t))
        { 
            res = vec2<f32>(t, h.y); 
            break;
        }
        t = t + h.x;
    }
    
    return res;
}

// https://iquilezles.org/articles/normalsSDF
fn calcNormal(pos: vec3<f32>) -> vec3<f32>
{
    var e = vec2<f32>(1.0,-1.0)*0.5773*0.0005;
    return normalize(e.xyy*map(pos + e.xyy).x + 
					 e.yyx*map(pos + e.yyx).x + 
					 e.yxy*map(pos + e.yxy).x + 
					 e.xxx*map(pos + e.xxx).x);
}

// https://iquilezles.org/articles/checkerfiltering
fn checkersGradBox(p: vec2<f32>, dpdx: vec2<f32>, dpdy: vec2<f32>) -> f32
{
    // filter kernel
    var w = abs(dpdx)+abs(dpdy) + 0.001;
    // analytical integral (box filter)
    let point5 = vec2<f32>(0.5);
    var i = 2.0*(abs(fract((p-point5*w)*0.5)-point5)-abs(fract((p+0.5*w)*0.5)-point5))/w;
    // xor pattern
    return 0.5 - 0.5*i.x*i.y;                  
}

// https://iquilezles.org/articles/rmshadows
fn calcSoftshadow(ro: vec3<f32>, rd: vec3<f32>, mint: f32, tmax: f32) -> f32
{
    // bounding volume
    var tp = (0.8-ro.y)/rd.y; 
    var ntmax = tmax;
    if(tp > 0.0) 
    {
        ntmax = min(ntmax, tp);
    }

    var res = 1.0;
    var t = mint;
    for(var i = 0; i < 24; i = i + 1)
    {
		var h = map(ro + rd*t).x;
        var s = clamp(8.0*h/t, 0.0, 1.0);
        res = min(res, s);
        t = t + clamp(h, 0.01, 0.2);
        if(res < 0.004 || t > ntmax) 
        {
            break;
        } 
    }
    res = clamp(res, 0.0, 1.0);
    return res*res*(3.0-(2.0*res));
}

// https://iquilezles.org/articles/nvscene2008/rwwtt.pdf
fn calcAO(pos: vec3<f32>, nor: vec3<f32>) -> f32
{
	var occ = 0.0;
    var sca = 1.0;
    for(var i = 0; i < 5; i = i + 1)
    {
        var h = 0.01 + 0.12*f32(i)/4.0;
        var d = map(pos + h*nor).x;
        occ = occ + (h-d)*sca;
        sca = sca * 0.95;
        if(occ > 0.35)
        {
            break;
        } 
    }
    return clamp(1.0 - 3.0 * occ, 0.0, 1.0) * (0.5 + 0.5 * nor.y);
}


fn render(ro: vec3<f32>, rd: vec3<f32>, rdx: vec3<f32>, rdy: vec3<f32>) -> vec3<f32>
{ 
    // background
    var col = vec3<f32>(0.7, 0.7, 0.9) - max(rd.y,0.0)*0.3;
    
    // raycast scene
    var res = raycast(ro,rd);
    var t = res.x;
	var m = res.y;
    if (m>-0.5)
    {
        var pos = ro + t*rd;
        var nor = select(calcNormal(pos), vec3<f32>(0.0,1.0,0.0), m < 1.5);
        var ref = reflect(rd, nor);
        
        // material        
        col = 0.2 + 0.2*sin(m*2.0 + vec3<f32>(0.0,1.0,2.0));
        var ks = 0.6;
        
        if(m < 1.5)
        {
            // project pixel footprint into the plane
            var dpdx = ro.y*(rd/rd.y-rdx/rdx.y);
            var dpdy = ro.y*(rd/rd.y-rdy/rdy.y);

            var f = checkersGradBox(3.0*pos.xz, 3.0*dpdx.xz, 3.0*dpdy.xz);
            col = 0.15 + f*vec3<f32>(0.15);
            ks = 0.4;
        }

        // lighting
        var occ = calcAO(pos, nor);
        
		var lin = vec3<f32>(0.0);

        // sun
        {
            var lig = normalize(vec3<f32>(-0.5, 0.4, -0.6));
            var hal = normalize(lig-rd);
            var dif = clamp(dot(nor, lig ), 0.0, 1.0);
                dif = dif * calcSoftshadow(pos, lig, 0.02, 2.5);
			var spe = pow(clamp(dot( nor, hal ), 0.0, 1.0), 16.0);
                spe = spe * dif;
                spe = spe * 0.04+0.96*pow(clamp(1.0-dot(hal,lig),0.0,1.0),5.0);
            lin = lin + col*2.20*dif*vec3<f32>(1.30,1.00,0.70);
            lin = lin +     5.00*spe*vec3<f32>(1.30,1.00,0.70)*ks;
        }
        // sky
        {
            var dif = sqrt(clamp(0.5+0.5*nor.y, 0.0, 1.0));
                dif = dif * occ;
            var spe = smoothStep(-0.2, 0.2, ref.y);
                spe = spe * dif;
                spe = spe * 0.04+0.96*pow(clamp(1.0+dot(nor,rd),0.0,1.0), 5.0);
                spe = spe * calcSoftshadow(pos, ref, 0.02, 2.5);
            lin = lin + col*0.60*dif*vec3<f32>(0.40,0.60,1.15);
            lin = lin +     2.00*spe*vec3<f32>(0.40,0.60,1.30)*ks;
        }


		col = lin;
        col = mix(col, vec3<f32>(0.7,0.7,0.9), 1.0-exp(-0.00002*t*t*t));
    }

	return clamp(col, vec3<f32>(0.0), vec3<f32>(1.0));
}


// Fragment shader
[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
    let base_coord = vec2<f32>(0.5, 0.5);

    var p = screen_coords(in, base_coord);
    var mp = mouse_pos()*6.;
    
    let t = 0.2 * time();
    var ta = vec3<f32>(0.0, 0.0, 0.0);
    var ro = ta + vec3<f32>(4.5*cos(0.1*time()+mp.x), 2.2, 4.5*sin(0.1*time()+mp.x))*mp.y;
    // camera-to-world transformation
    var ca = camera(ro, ta, 0.0);

    // focal length
    let fl = 2.5;
    
    // ray direction
    var rd = ca * normalize(vec3<f32>(p, fl));
        // ray differentials
    var px = screen_coords(in, base_coord + vec2<f32>(0.0001,0.0));
    var py = screen_coords(in, base_coord + vec2<f32>(0.0,0.0001));
    var rdx = ca * normalize(vec3<f32>(px, fl));
    var rdy = ca * normalize(vec3<f32>(py, fl));
    
    // render	
    var col = render(ro, rd, rdx, rdy);

    // gamma
    col = pow(col, vec3<f32>(0.64545));


    return vec4<f32>(col, 1.);
}
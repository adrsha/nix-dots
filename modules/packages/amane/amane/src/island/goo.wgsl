// Normalized weights for the original 40-column quadrature. Constant-folded
// rather than evaluating exp and summing identical weights per fragment.
const WEIGHTS: array<f32,40> = array<f32,40>(
    0.0008324042,
    0.0012764245,
    0.0019137464,
    0.0028054468,
    0.0040211294,
    0.0056353700,
    0.0077219182,
    0.0103456155,
    0.0135523886,
    0.0173581617,
    0.0217380202,
    0.0266173381,
    0.0318667393,
    0.0373025918,
    0.0426941885,
    0.0477778819,
    0.0522773297,
    0.0559278692,
    0.0585021093,
    0.0598333268,
    0.0598333268,
    0.0585021093,
    0.0559278692,
    0.0522773297,
    0.0477778819,
    0.0426941885,
    0.0373025918,
    0.0318667393,
    0.0266173381,
    0.0217380202,
    0.0173581617,
    0.0135523886,
    0.0103456155,
    0.0077219182,
    0.0056353700,
    0.0040211294,
    0.0028054468,
    0.0019137464,
    0.0012764245,
    0.0008324042
);
// One continuous field over the island AND the gap AND the popup. No scissors
// per shape. Integrate Gaussian alpha vertically, then quadrature horizontally:
// unlike a sparse 2D sample grid this cannot stamp a lattice into the neck.
fn distance_round(p: vec2<f32>, rect: vec4<f32>, r: f32) -> f32 {
    let q = abs(p - rect.xy - rect.zw * 0.5) - rect.zw * 0.5 + r;
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
}
fn cdf(v: f32) -> f32 {
    let x = abs(v) * 0.70710678;
    let t = 1.0 / (1.0 + 0.3275911 * x);
    let erf = 1.0 - (((((1.061405429*t-1.453152027)*t)+1.421413741)*t-0.284496736)*t+0.254829592)*t*exp(-x*x);
    return 0.5 + 0.5 * sign(v) * erf;
}
fn span(x: f32, rect: vec4<f32>, radius: f32) -> vec2<f32> {
    if x < rect.x || x > rect.x+rect.z { return vec2<f32>(0.0); }
    let r = min(radius, min(rect.z,rect.w)*0.5);
    let inset_x = max(abs(x-rect.x-rect.z*0.5)-(rect.z*0.5-r),0.0);
    let inset_y = r - sqrt(max(r*r-inset_x*inset_x,0.0));
    return vec2<f32>(rect.y+inset_y,rect.y+rect.w-inset_y);
}
fn integral(interval: vec2<f32>, y: f32, sigma: f32) -> f32 {
    if interval.y <= interval.x { return 0.0; }
    return cdf((interval.y-y)/sigma) - cdf((interval.x-y)/sigma);
}
@fragment
fn main(input: Corner) -> @location(0) vec4<f32> {
    let p = input.uv * size;
    let sigma = values[2].z;
    let distance = min(distance_round(p,values[0],values[2].x), distance_round(p,values[1],values[2].y));
    let aa = max(fwidth(distance)*0.5,0.5);
    if sigma < 0.1 {
        return vec4<f32>(values[3].rgb,(1.0-smoothstep(-aa,aa,distance))*values[3].a);
    }
    var alpha = 0.0;
    // Far from both boundaries the thresholded Gaussian is unambiguously
    // empty/full. Keep the exact original integration in the entire neck and
    // edge band; do not lower sample count or approximate the visible shape.
    let band = 4.0*sigma + 2.0;
    if distance < -band {
        alpha = 1.0;
    } else if distance <= band {
        // Midpoint integration, 40 columns across +/- 3 Gaussian sigmas.
        for (var i=0; i<40; i=i+1) {
            let dx = (f32(i)+0.5)*0.15-3.0;
            let x = p.x+dx*sigma;
            let a = span(x,values[0],values[2].x);
            let b = span(x,values[1],values[2].y);
            let overlap = vec2<f32>(max(a.x,b.x),min(a.y,b.y));
            alpha += WEIGHTS[i] * (integral(a,p.y,sigma)+integral(b,p.y,sigma)-integral(overlap,p.y,sigma));
        }
    }
    let edge = max(fwidth(alpha)*0.65,0.004);
    let level = values[2].w;
    return vec4<f32>(values[3].rgb,smoothstep(level-edge,level+edge,alpha)*values[3].a);
}

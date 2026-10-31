// All temporal frequencies are multiples of .01, so this wrap has no seam.
export const HUB_FLOW_PERIOD = 200 * Math.PI;

// Rotate the interior at one speed. Only the bounded sine term varies by
// radius: multiplying elapsed time by a radial fade would keep winding the
// galaxy tighter. Blend the stationary rim separately from the flowing image.
export const hubFlowMapFragment = /* glsl */`
  #ifdef USE_MAP
    vec2 center = vec2(626.0 / 1254.0, 603.0 / 1254.0);
    vec2 p = vMapUv - center;
    float radius = length(p) / (3.685 * 70.5 / 1254.0);
    float interior = 1.0 - smoothstep(0.82, 0.98, radius);
    float angle = uHubTime * 0.06 + 0.10 * sin(radius * 8.0 - uHubTime * 0.30);
    float c = cos(angle), s = sin(angle);
    vec2 uv = center + mat2(c, -s, s, c) * p;
    vec4 texel = mix(texture2D(map, vMapUv), texture2D(map, uv), interior);
    float wave = sin(radius * 51.0 - uHubTime * 1.80) * 0.018 * interior;
    texel.rgb += vec3(0.05, 0.3, 0.65) * wave;
    diffuseColor *= texel;
  #endif
`;

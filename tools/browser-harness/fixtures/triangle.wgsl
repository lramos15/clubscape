struct Parameters {
  values: vec4f,
}
@group(0) @binding(0) var<uniform> parameters: Parameters;

@vertex fn vertexMain(@builtin(vertex_index) index: u32) -> @builtin(position) vec4f {
  let vertices = array<vec2f, 3>(vec2f(-1.0, -1.0), vec2f(3.0, -1.0), vec2f(-1.0, 3.0));
  return vec4f(vertices[index], 0.0, 1.0);
}

@fragment fn fragmentMain(@builtin(position) position: vec4f) -> @location(0) vec4f {
  let split = 0.5 + 0.1 * sin(parameters.values.y);
  let color = select(vec3f(0.12, 0.3, 0.65), vec3f(0.9, 0.65, 0.12), position.x / parameters.values.z > split);
  return vec4f(color * parameters.values.x, 1.0);
}

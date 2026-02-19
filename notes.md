TODO:
- Go through each section of https://google.github.io/tour-of-wgsl/
- Read an example program
  - Cross reference with:
  - https://webgpufundamentals.org/webgpu/lessons/webgpu-fundamentals.html
    - https://webgpufundamentals.org/webgpu/lessons/webgpu-perspective-projection.html

Notes on WSGL:
- Functions are not allowed to be recursive, which makes sense since it's a shader language
- There can be multiple `@vertex`, `@fragment` or `@compute` entry points
- Base types: `u32`, `f32`, `i32`, `bool`. All the integer types are implicitly converted
  from `abstract float` or `abstract int`, which are 64 bit integer values evaluated at compile time.
- WGSL matrices are column-major
- Static sized array type `array<T, N>`, runtime sized array type: `array<T>`
  Runtime sized arrays can only be used with storage buffer ressources and can't be passed around
- Writing to storage buffers leads to data races (by definition). Mark variables as `atomic` so that
  the WGSL type system prevents non atomic access to the data. Atomic guarantees that a *single* memory
  word will be modified in some order.
- Atomic operations only work on `i32` or `u32` variables in the `storage` or `workgroup` address space.
  Atomic variables cannot be directly used, instead use functions that act on a pointer to the atomic var.
- [Pointer syntax](https://google.github.io/tour-of-wgsl/types/pointers/specifying/)

Notes on WebGPU:
- Bind groups: Store buffers for input to the pipeline
- Must create the depth buffer yourself
High performance molecule visualizer built in Rust and wgpu

Ressources:
- [Chemical table file](https://en.wikipedia.org/wiki/Chemical_table_file)
- [SDF File format guidance](https://www.nonlinear.com/progenesis/sdf-studio/v0.9/faq/sdf-file-format-guidance.aspx)
- [Learn WGPU](https://sotrh.github.io/learn-wgpu/beginner/tutorial1-window/#boring-i-know)
- [Distance functions](https://iquilezles.org/articles/distfunctions/)

Interesting tangents:
- [Contribute to glam?](https://github.com/bitshifter/glam-rs)
- Write a blog article on utilizing signed distance fields to draw shapes

Notes:
- Port to winit 0.31.0 once it comes out of beta, although the new api is different from the stable one

Part 1:
- [x] Basic SDF parser with tests

- [ ] WGPU usage:
  - [x] Add a depth buffer
  - [x] Render multiple items using instancing
  - [x] Setup basic camera movement
  - [x] MSAA antialiasing
  - [ ] Use signed distance fields to render a sphere and a cylinder
  - [ ] Phong lighting
  - [ ] Refactor the wgpu usage. Write a comment explaining the overall architecture (how we are drawing objects)
  - [ ] Think about how compounds will be drawn, investigate what PubChem does

- [ ] Render the atoms and bonds in a molecule
  - [ ] Ball and stick model
  - [ ] Space filling model
    - [ ] Using van der Waal radii
    - [ ] Each atom/bond should have their own color, using the CPK colorscheme

- [ ] Nice graphics:
  - [ ] Shader which highlights edges and provides nice phong lighting in the scene
  - [ ] 3D camera panning and zooming in/out

Part 2:
- mmCIF parser, test with real data from the Protein Data Bank

- Full structural hierarchy: Model -> Chain -> Residue -> Atom

- Infer bond using covalent radii and distance thresholds when needed

- Rendering protein using ribbons, also detecting and rendering Helix, Beta sheets, Coils, etc

- Performance engineering to render thousands of atoms while maintaining 60 fps without a beefy gpu

Part 3:
- Port to WASM, add a basic UI to select/search for files, ask for feedback from actual scientists
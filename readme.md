High performance molecule visualizer built in Rust and wgpu

Ressources:
- [Chemical table file](https://en.wikipedia.org/wiki/Chemical_table_file)
- [SDF File format guidance](https://www.nonlinear.com/progenesis/sdf-studio/v0.9/faq/sdf-file-format-guidance.aspx)
- [Learn WGPU](https://sotrh.github.io/learn-wgpu/beginner/tutorial1-window/#boring-i-know)
- [Distance functions](https://iquilezles.org/articles/distfunctions/)
- [Awesome Chemistry Datasets](https://github.com/kjappelbaum/awesome-chemistry-datasets)
- [Covalent radii revisited](https://www.researchgate.net/publication/5373706_Covalent_radii_revisited)

Interesting tangents:
- [Contribute to glam?](https://github.com/bitshifter/glam-rs)

Notes:
- Port to winit 0.31.0 once it comes out of beta, although the new api is different from the stable one

Part 1:
- [x] Basic SDF parser with tests

- [ ] WGPU usage:
  - [x] Add a depth buffer
  - [x] Render multiple items using instancing
  - [x] Setup basic camera movement
  - [x] MSAA antialiasing
  - [x] Use signed distance fields to render a sphere and a cylinder
  - [x] Phong lighting
  - [x] Refactor the wgpu usage. Write a comment explaining the overall architecture (how we are drawing objects)
  - [x] Think about how compounds will be drawn, investigate what PubChem does

- [x] Nice graphics:
  - [x] Shader which highlights edges and provides nice phong lighting in the scene
  - [x] 3D camera panning and zooming in/out

- [ ] Improve the visualization
  - [ ] Integrate with egui to render fps
  - [ ] View a new molecule using a file dialog
  - [ ] Optimize the SDF rendering (what we have is way too slow)
  - [ ] Improve camera movement, panning and zooming


- [ ] Render the atoms and bonds in a molecule
  - [x] Map the parsed `Compound` into `Vec<Shape>`
  - [ ] Feature complete ball and stick model
  - [ ] Feature complete space filling model


Part 2:
- mmCIF parser, test with real data from the Protein Data Bank

- Full structural hierarchy: Model -> Chain -> Residue -> Atom

- Infer bond using covalent radii and distance thresholds when needed

- Rendering protein using ribbons, also detecting and rendering Helix, Beta sheets, Coils, etc

- Performance engineering to render thousands of atoms while maintaining 60 fps without a beefy gpu

Part 3:
- You know what would be really cool? Being able to step through and simulate chemical reactions.
  In other words, visualize a chemical equation.
- Port to WASM, add a basic UI to select/search for files, ask for feedback from actual scientists

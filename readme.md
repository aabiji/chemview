High performance molecule visualizer built in Rust and wgpu

Part 1:
- SDF file parser, test with real data from PubChem

- Render the atoms and bonds in a molecule
  - Ball and stick model
  - Space filling model
    - Using van der Waal radii
  - Each atom/bond should have their own color, using the CPK colorscheme

- Nice graphics:
  - Shader which highlights edges and provides nice phong lighting in the scene
  - 3D camera panning and zooming in/out

Part 2:
- mmCIF parser, test with real data from the Protein Data Bank

- Full structural hierarchy: Model -> Chain -> Residue -> Atom

- Infer bond using covalent radii and distance thresholds when needed

- Rendering protein using ribbons, also detecting and rendering Helix, Beta sheets, Coils, etc

- Performance engineering to render thousands of atoms while maintaining 60 fps without a beefy gpu

Part 3:
- Port to WASM, add a basic UI to select/search for files

High performance molecule visualizer built in Rust and wgpu

Ressources:
- [Chemical table file](https://en.wikipedia.org/wiki/Chemical_table_file)
- [SDF File format guidance](https://www.nonlinear.com/progenesis/sdf-studio/v0.9/faq/sdf-file-format-guidance.aspx)
- [Learn WGPU](https://sotrh.github.io/learn-wgpu/beginner/tutorial1-window/#boring-i-know)
- [Covalent radii revisited](https://www.researchgate.net/publication/5373706_Covalent_radii_revisited)
- [Atomic Radius in the Periodic Table of Elements](https://pubchem.ncbi.nlm.nih.gov/ptable/atomic-radius/)
- [OpenGL Cylinder, Prism & Pipe](https://www.songho.ca/opengl/gl_cylinder.html)
- [OpenGL Sphere](https://www.songho.ca/opengl/gl_sphere.html)
- [Claude](https://claude.ai/), mainly used for drafting an initial implementation roadmap

- [PDB-101](https://pdb101.rcsb.org/)
- [PDBx/mmCIF User Giude](https://mmcif.wwpdb.org/docs/user-guide/guide.html)

Part 1 -> Small molecule viewer:
- [x] Basic SDF parser with tests

- [x] WGPU usage:
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

- [x] Improve the visualization
  - [x] Scrap the SDF rendering, instead draw shapes by generating the meshes at startup and instancing
  - [x] Integrate with egui to render fps
  - [x] Improve camera movement, panning and zooming

- [x] Render the atoms and bonds in a molecule
  - [x] Map the parsed `Compound` into `Vec<Shape>`
  - [x] Feature complete ball and stick model
  - [x] Feature complete space filling model

Part 1.5 -> Improve UX:
- [x] Fixed position camera, mouse rotates the compound itself, not the scene
- [x] Add a ui to manipulate the compound in focus
- [ ] Loading should be done on a seperate thread. Updating the compound's view type should be
      done by creating shape buffers for all view types, then switching them out during runtime.

Part 2 -> Render proteins:
- [ ] Parse mmCIF files
  - [ ] Get test data from the Protein Data Bank
  - [ ] Answer questions:
    - Are bonds explicit like in SDF, or inferrred?
    - Are H atoms inferred?
  - [ ] KD-trees for spatial queries???

- Render the protein
  - [ ] Space filling model
    - [ ] The protein will have several orders of magnitude more atoms than
          the simple compounds, so research several techniques to optimize rendering
    - [ ] Implement frustrum culling: don't render objects outside of the camera's view
    - [ ] Level of detail: switch between different representations based off of the zoom level

  - [ ] Cartoon model
    - [ ] Render ribbons for helices, arrows for beta strands, tubes for loops. To do this,
          I need to know which *residues* are alpha helicies, beta shets or coils.
          Implement the **DSSP** (Define Secondary STructure of Proteins) to figure that out
    - [ ] Use the alpha carbon atom from each residue to use as positions for control points.
          Explore using Catmull-Rom or cubix Hermite spline through the control points
    - [ ] Build the ribbon geometry using a Fresnet-Serret frame. Add arrow heads for beta strands

  - [ ] Surface model
      - ???

Part 3 -> Wild ideas:
- Visualize a chemical reaction as it happens?

- Search for compounds using `https://pubchem.ncbi.nlm.nih.gov/rest/pug/compound/name/aspirin/SDF?record_type=3d`

- Port to WASM, host on github pages, ask for feedback from actual scientists

Protein visualizer built in Rust and wgpu.

### Roadmap

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
  - [x] Feature complete wireframe model

- [x] Better UX:
  - [x] Fixed position camera, mouse rotates the compound itself, not the scene
  - [x] Add a ui to manipulate the compound in focus
    - [x] File path input
    - [x] Dropdown to change view mode
  - [x] Load compounds on a separate thread

Part 2 -> Protein renderer:
- [x] Parse mmCIF files
  - [x] Basic mmCIF file parsing into structured types
  - [x] mmap the file and parse by streaming the content
  - [x] Organize the data into Chains and Residues
  - [x] Abstract away the file format used. Use an interface that loads the file, then the file to output a mesh,
        use that mesh data (not tied to any semantic meaning) to do instance rendering
  - [ ] Refactor the mmCIF parser. It should not be 600+ lines, refactor to reduce string allocations, make it faster and make it simpler/more ergonomic

- Improve existing rendering
  - [ ] Add global illumination to avoid shadows and dim areas of the compound
  - [ ] Initially position the camera far back enough in order to see the entire compound at once
  - [ ] Constrain camera movement
  - [ ] Adjust the projection matrix to keep the entire compound in view when zoomed fully out
  - [ ] Dynamically allocate the storage buffer for vertices (still with a hard limit)
  - [ ] Cache the mesh data for different view types
    - [ ] Optionally filter H and H20 during mmcif parsing
  - [ ] Refactor the meshing algorithms
    - [ ] Generate spheres with less triangles
    - [ ] Generate a [capsule](https://gamedev.stackexchange.com/questions/162426/how-to-draw-a-3d-capsule)
          instead of an open cylinder
  - [ ] Add frustrum culling: Don't render vertices outside of the camera's view
  - [ ] Refactor the way that instancing is done: having to keep track of the number of spheres is a weird decision

- [ ] Infer the compound
  - [x] Parse bonds from the `chem_comp_bond` table
  - [ ] Read the CCD file to get residue bonds that aren't present in the mmcif file
  - [ ] Read the CCD file to get residue atoms (like H) that aren't present in the mmcif file

- [ ] Render proteins
  - Read this [paper](https://link.springer.com/article/10.1186/1472-6807-9-19) and take detailed notes

Part 3 -> Extra ideas:
- Render the protein using a surface model

- Make this an library that can be used by other crates??

- Click on meshes to highlight and get info on different chains, residues, ligands, etc. Optionally filter out atoms/molecules (ex: H, H20)

- Visualize a chemical reaction as it happens?

- Search for compounds using `https://pubchem.ncbi.nlm.nih.gov/rest/pug/compound/name/aspirin/SDF?record_type=3d`

- Port to WASM, host on github pages, ask for feedback from actual scientists

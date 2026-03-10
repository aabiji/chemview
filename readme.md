High performance molecule visualizer built in Rust and wgpu

### Terminology
*Residue*: A singe monomer in a polymer chain. Monomers are compose of many individual atoms.
*Ligand*: Any non polymer molecule bound to the main structure. (HETATM)
*Oligosaccharide*: Carbohydrate chain 3-10 monomers long.
*Polysaccharide*: Carbohydrate chain 10+ monomers long.
*Glycans*: Oligosaccharides or polysaccharides that form part of the glycocalix (dense carbohydrate coating on the exterior of the cell membrane) and help cells to identify themselves and interact with their environment.

### Open questions
- If all you have are the atoms, how are you supposed to infer bonds? Can you even infer bonds?
- What do we actually parse from the file to render the primary, secondary, tertiary and quaternary structure of the protein?
- B-Trees are supposed to be good for range queries, so how do they work?

### Ressources

- [Chemical table file](https://en.wikipedia.org/wiki/Chemical_table_file)
- [SDF File format guidance](https://www.nonlinear.com/progenesis/sdf-studio/v0.9/faq/sdf-file-format-guidance.aspx)
- [Learn WGPU](https://sotrh.github.io/learn-wgpu/beginner/tutorial1-window/#boring-i-know)
- [Covalent radii revisited](https://www.researchgate.net/publication/5373706_Covalent_radii_revisited)
- [Atomic Radius in the Periodic Table of Elements](https://pubchem.ncbi.nlm.nih.gov/ptable/atomic-radius/)
- [OpenGL Cylinder, Prism & Pipe](https://www.songho.ca/opengl/gl_cylinder.html)
- [OpenGL Sphere](https://www.songho.ca/opengl/gl_sphere.html)

- [PDB-101](https://pdb101.rcsb.org/)
- [PDBx/mmCIF User Giude](https://mmcif.wwpdb.org/docs/user-guide/guide.html)
- [Structures of Human Sequences](https://www.rcsb.org/search?q=rcsb_entity_source_organism.ncbi_scientific_name:Homo%20sapiens)
- [Rendering techniques for proteins](https://www.frontiersin.org/journals/computer-science/articles/10.3389/fcomp.2021.642172/full)

- [Protein secondary structures](https://chemistrytalk.org/protein-secondary-structures/)
- [What are proteins?](https://chem.libretexts.org/Bookshelves/Introductory_Chemistry/Introduction_to_Organic_and_Biochemistry_(Malik)/07%3A_Proteins/7.01%3A_What_are_proteins)

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

Part 1.5 -> Improve UX:
- [x] Fixed position camera, mouse rotates the compound itself, not the scene
- [x] Add a ui to manipulate the compound in focus

Part 2 -> Render proteins:
- [ ] Parse mmCIF files
  - [x] mmap the file and do the first pass: Parse key/value and tables into `Block`s
  - [ ] Filter the blocks that are needed and parse the `_chem_comp_atom`, `_atom_site` and `_chem_bond` blocks to get atom and bond info

  - [ ] Render those parse atoms and bonds
    - [ ] Loading should be done on a seperate thread. Updating the compound's view type should be
          done by creating shape buffers for all view types, then switching them out during runtime.
    - [ ] The ball and stick renderer should color the bonds based off of which atom is attached
    - [ ] The ball and stick renderer should not render spheres (or make the spheres much, much smaller)
    - [ ] The renderer should filter out H, since it clutters the view

  - [ ] Abstract away the file format used. Use an interface that loads the file, then the file to output a mesh,
        use that mesh data (not tied to any semantic meaning) to do instance rendering

- Render the protein in different ways
  - [ ] Wirefram diagram: draw a line for each of the covalent bonds formed between atoms

  - [ ] Space filling model
    - [ ] The protein will have several orders of magnitude more atoms than
          the simple compounds, so research several techniques to optimize rendering
    - [ ] Implement frustrum culling: don't render objects outside of the camera's view
    - [ ] Level of detail: switch between different representations based off of the zoom level

  - [ ] Backbone and ribbon diagram: draw a tube that connects the positions of each amino acid.
        Add a spring shaped ribbon to represent alpha helices and a flat arrow to represent beta strands.
    - [ ] Render ribbons for helices, arrows for beta strands, tubes for loops. To do this,
          I need to know which *residues* are alpha helicies, beta shets or coils.
          Implement the **DSSP** (Define Secondary STructure of Proteins) to figure that out
    - [ ] Use the alpha carbon atom from each residue to use as positions for control points.
          Explore using Catmull-Rom or cubix Hermite spline through the control points
    - [ ] Build the ribbon geometry using a Fresnet-Serret frame. Add arrow heads for beta strands
    - [ ] Overlay the ball and stick Overton the ribbon rendering
    - [ ] Render disulfide bridges

  - [ ] Surface model
      - ???

Part 3 -> Wild ideas:
- Visualize a chemical reaction as it happens?

- Search for compounds using `https://pubchem.ncbi.nlm.nih.gov/rest/pug/compound/name/aspirin/SDF?record_type=3d`

- Port to WASM, host on github pages, ask for feedback from actual scientists

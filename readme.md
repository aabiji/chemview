Molecule visualizer built in Rust and wgpu

### Notes
*Residue*: Different name for an amino acid.

*Ligand*: Any non polymer molecule bound to the main structure. (HETATM)

*Oligosaccharide*: Carbohydrate chain 3-10 monomers long.

*Polysaccharide*: Carbohydrate chain 10+ monomers long.

*Glycans*: Oligosaccharides or polysaccharides that form part of the glycocalix (dense carbohydrate coating on the exterior of the cell membrane) and help cells to identify themselves and interact with their environment.

*Protein structures*
- Primary:
  - Amino acids linked together by peptide bonds (C-N bond -> the N-terminus side with the C-terminus side).
  - The peptide bond has two resonance structures, making there be no free rotation
    ![](https://chem.libretexts.org/@api/deki/files/431969/clipboard_e91b1f279848763bb8eb3efbaa73339d7.png?revision=1)
    around the peptide bond.
  - Cysteine has `HS` for its `R` group, and when oxygenated, those R group can form disulfide bonds (S-S).
  - Amino acids are read from the N terminus side to the C terminus side.

- Secondary
  - Helixes:
    - Alpha helix: The amino acids in the chain are already oriented in 3D space such that C=O in
      the *i*th amino acid points directly towards N-H of the *i + 4*th amino acid. So, an H bond can form,
      giving the structure stability. The H bond strengthened a connection that's already implied by the chain's geometry.
      The `R` groups branch off of the main backbone chain.
    - 3-10 helix: An H bond forms between the carboxyl O of the *i*th amino acid and the amine N *i + 3*th amino acid
    - Pi helix: An H bond forms between the carboxyl O of the *i*th amino acid and the amine N *i + 5*th amino acid
    - H bonds are **intrastrand**. Rendered as coils.
    - *Triple helix*: A set of 3 identical helices with the axis, differing only by a translation on that axis.
      Seen in collagen for example.

  - Beta sheet: The amino acid chain is pulled nearly straight, and the up/down zig-zag pattern
    (aligned parallel (N terminus on the same side) or antiparallel (N terminus on opposite sides))
    comes from the tetrahedral shape at each alpha carbon. H bonds are **interstrand**, from the
    carboxyl O in the amino acid of one sheet, to the amine N in the amino acid of
    another sheet. Rendered as a flat ribbon with an arrowhead pointed towards the N-terminus.

  - *Random coils*: Organized but not repeating amino acid structures between alpha helixes and beta sheets.
    Represented using lines ![](https://upload.wikimedia.org/wikipedia/commons/3/30/Insulin_1AI0_animation.gif).

- Tertiary
  - The different interactions that hold a ![polypeptide chain stable](https://chem.libretexts.org/@api/deki/files/432385/clipboard_eb73f08683beb5308f9ebeb05bc9bd823.png?revision=1)
    in 3D space include disulfide bonds, salt bridge, coordinate (covalent) bonds, hydrogen bonding, hydrophobic interactions, etc.

- Quaternary
  - A collection of polypeptide chains (not covalently bonded to each other) held together by the tertiary interactions described above.

*Model types*
- Space filling: Represents the physical volume of each of the atoms in a compound.
- Wireframe: Represents the bonds between atoms.
- Ball and stick: Wireframe with spheres to represent atoms.

- ![](https://cdn.ncbi.nlm.nih.gov/pmc/blobs/8d18/7203745/a81d112126a1/btaa072f1.jpg)
```
  `_entity.id`                                -> `_struct_asym.entity_id`               : map an entity to its chains
  `_struct_asym.id`                           -> `_atom_site.label_asym_id`             : map a chain to its atoms
  `_entity_poly_seq.num`                      -> `_atom_site.label_seq_id`              : map a residue to its atoms
  `_struct_conf.beg_label_asym_id`            -> `_struct_asym.id`                      : map the start of a helix to a chain
  `_struct_conf.beg_label_seq_id`             -> `_entity_poly_seq.num`                 : map the start of a helix to a squence
  `_struct_sheet_range.beg_label_asym_id`     -> `_struct_asym.id`                      : map the start of a beta sheet to a chain
  `_struct_sheet_range.beg_label_seq_id`      -> `_entity_poly_seq.num`                 : map the start of a beta sheet to a squence
  `_pdbx_struct_assembly.id`                  -> `_pdbx_struct_assembly_gen.assembly_id`: map an assembly to the generator operations list
  `_pdbx_struct_assembly_gen.asym_id_list`    -> `_struct_asym.id`                      : map the assembly operation to the chains to transform
  `_pdbx_struct_assembly_gen.oper_expression` -> `_pdbx_struct_oper_list.id`            : map the assembly operation to the actual transformation matrices
  `_atom_site`: (label_entity_id, label_asym_id, label_seq_id)
```
- Visualization pipeline:
  1. Parse files (SDF, mmCIF) into `Structure`
  2. Tessellate `Structure` into `Shape`s
  3. Convert `Shape`s to mesh data for rendering


### Ressources

- [Chemical table file](https://en.wikipedia.org/wiki/Chemical_table_file)
- [SDF File format guidance](https://www.nonlinear.com/progenesis/sdf-studio/v0.9/faq/sdf-file-format-guidance.aspx)
- [Learn WGPU](https://sotrh.github.io/learn-wgpu/beginner/tutorial1-window/#boring-i-know)
- [Covalent radii revisited](https://www.researchgate.net/publication/5373706_Covalent_radii_revisited)
- [Atomic Radius in the Periodic Table of Elements](https://pubchem.ncbi.nlm.nih.gov/ptable/atomic-radius/)
- [OpenGL Cylinder, Prism & Pipe](https://www.songho.ca/opengl/gl_cylinder.html)
- [OpenGL Sphere](https://www.songho.ca/opengl/gl_sphere.html)
- [What are proteins?](https://chem.libretexts.org/Bookshelves/Introductory_Chemistry/Introduction_to_Organic_and_Biochemistry_(Malik)/07%3A_Proteins/7.01%3A_What_are_proteins)
- [Secondary Structure and Loops](https://bio.libretexts.org/Bookshelves/Biochemistry/Fundamentals_of_Biochemistry_(Jakubowski_and_Flatt)/01%3A_Unit_I-_Structure_and_Catalysis/04%3A_The_Three-Dimensional_Structure_of_Proteins/4.02%3A_Secondary_Structure_and_Loops)

-- [Structures of Human Sequences](https://www.rcsb.org/search?q=rcsb_entity_source_organism.ncbi_scientific_name:Homo%20sapiens)
- [Rendering techniques for proteins](https://www.frontiersin.org/journals/computer-science/articles/10.3389/fcomp.2021.642172/full)

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
- [x] Parse mmCIF files
  - [x] Basic mmCIF file parsing into structured types
  - [x] mmap the file and parse by streaming the content
  - [x] Organize the data into Chains and Residues
  - [x] Abstract away the file format used. Use an interface that loads the file, then the file to output a mesh,
        use that mesh data (not tied to any semantic meaning) to do instance rendering
  - [x] Compounds should be loaded on a separate thread

- [ ] Render proteins
  - [x] Wirefram diagram
  - [x] Space filling diagram
  - [ ] Include a light behind the origin
  - [ ] Draw [capsules](https://gamedev.stackexchange.com/questions/162426/how-to-draw-a-3d-capsule) instead of open ended cylinders
  - [x] Perform bond inference to establish bonds between every single atom in the chain
  - [ ] Add dotted lines for hydrogen bonds and disulfide bonds
  - [ ] Implement frustrum culling: don't render objects outside of the camera's view
  - [ ] Level of detail: switch between different representations based off of the zoom level (for massive molecules)

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

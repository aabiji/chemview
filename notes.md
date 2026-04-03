- [Chemical table file](https://en.wikipedia.org/wiki/Chemical_table_file)
- [SDF File format guidance](https://www.nonlinear.com/progenesis/sdf-studio/v0.9/faq/sdf-file-format-guidance.aspx)
- [Learn WGPU](https://sotrh.github.io/learn-wgpu/beginner/tutorial1-window/#boring-i-know)
- [Covalent radii revisited](https://www.researchgate.net/publication/5373706_Covalent_radii_revisited)
- [Atomic Radius in the Periodic Table of Elements](https://pubchem.ncbi.nlm.nih.gov/ptable/atomic-radius/)
- [OpenGL Cylinder, Prism & Pipe](https://www.songho.ca/opengl/gl_cylinder.html)
- [OpenGL Sphere](https://www.songho.ca/opengl/gl_sphere.html)
- [What are proteins?](https://chem.libretexts.org/Bookshelves/Introductory_Chemistry/Introduction_to_Organic_and_Biochemistry_(Malik)/07%3A_Proteins/7.01%3A_What_are_proteins)
- [Secondary Structure and Loops](https://bio.libretexts.org/Bookshelves/Biochemistry/Fundamentals_of_Biochemistry_(Jakubowski_and_Flatt)/01%3A_Unit_I-_Structure_and_Catalysis/04%3A_The_Three-Dimensional_Structure_of_Proteins/4.02%3A_Secondary_Structure_and_Loops)
- [Generating an icosphere with code](https://blog.lslabs.dev/posts/generating_icosphere_with_code)
- [ProteinShader: Illustrative rendering of macromolecules](https://link.springer.com/article/10.1186/1472-6807-9-19)

---

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

- Visualization pipeline:
  1. Parse files (SDF, mmCIF) into `Structure`
  2. Tessellate `Structure` into `Shape`s
  3. Convert `Shape`s to mesh data for rendering

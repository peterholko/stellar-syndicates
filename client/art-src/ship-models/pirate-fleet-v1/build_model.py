"""Build the selected reference-based pirate mesh, without Blender."""
from mesh_core import SPEC, export
from designs import DESIGNS

if __name__ == '__main__':
    DESIGNS[SPEC['id']]()
    export()

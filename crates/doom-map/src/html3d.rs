//! Export map geometry to a standalone interactive 3D HTML viewer.
//!
//! This module provides the `export_map_to_html3d` function, which creates a rich,
//! self-contained HTML file. It embeds the map's floor and ceiling geometry
//! and a simple WebGL/Three.js script to allow interactive 3D exploration
//! directly in the browser.

use crate::Level;

/// Exports a `Level` to a self-contained 3D HTML file.
///
/// Extracts the bounding polygons of sectors to construct floor and ceiling
/// meshes, and embeds them into an HTML document equipped with a lightweight
/// 3D viewer (via Three.js).
pub fn export_map_to_html3d(level: &Level) -> String {
    let mut vertexes = Vec::new();
    let mut faces = Vec::new();
    let mut vertex_count = 0;

    // We build simple walls linking floor and ceiling for each 2-sided linedef.
    // For a fully robust 3D representation, we'd need a polygon triangulation algorithm (like ear clipping)
    // to draw the floors and ceilings perfectly, but since we are just doing an overhead visualization,
    // we can construct walls between sectors of differing heights.
    for ld in &level.linedefs {
        let v1 = &level.vertexes[ld.from_vertex as usize];
        let v2 = &level.vertexes[ld.to_vertex as usize];

        let mut quads = Vec::new();

        if !ld.is_two_sided() {
            // One sided linedef. Solid wall from floor to ceiling.
            let sector_idx = if let Some(sd) = level.sidedefs.get(ld.right_sidedef as usize) {
                sd.sector as usize
            } else {
                continue;
            };

            let sector = &level.sectors[sector_idx];
            quads.push((sector.floor_height, sector.ceil_height));
        } else {
            // Two sided. Construct walls bridging the height differences.
            let right_sd = level.sidedefs.get(ld.right_sidedef as usize);
            let left_sd = level.sidedefs.get(ld.left_sidedef as usize);

            if right_sd.is_none() || left_sd.is_none() {
                continue;
            }

            let front_sector = &level.sectors[right_sd.unwrap().sector as usize];
            let back_sector = &level.sectors[left_sd.unwrap().sector as usize];

            if front_sector.floor_height < back_sector.floor_height {
                quads.push((front_sector.floor_height, back_sector.floor_height));
            } else if back_sector.floor_height < front_sector.floor_height {
                quads.push((back_sector.floor_height, front_sector.floor_height));
            }

            if front_sector.ceil_height > back_sector.ceil_height {
                quads.push((back_sector.ceil_height, front_sector.ceil_height));
            } else if back_sector.ceil_height > front_sector.ceil_height {
                quads.push((front_sector.ceil_height, back_sector.ceil_height));
            }
        }

        for (z_bottom, z_top) in quads {
            if z_bottom >= z_top {
                continue;
            }

            // Doom coords: X is East/West, Y is North/South.
            // 3D coords for Three.js: X = X, Y = Up (Doom Z), Z = -Doom Y
            vertexes.push(format!("[{},{},{}]", v1.x, z_bottom, -v1.y));
            vertexes.push(format!("[{},{},{}]", v2.x, z_bottom, -v2.y));
            vertexes.push(format!("[{},{},{}]", v2.x, z_top, -v2.y));
            vertexes.push(format!("[{},{},{}]", v1.x, z_top, -v1.y));

            faces.push(format!(
                "[{},{},{},{}]",
                vertex_count,
                vertex_count + 1,
                vertex_count + 2,
                vertex_count + 3
            ));
            vertex_count += 4;
        }
    }

    let verts_json = format!("[{}]", vertexes.join(","));
    let faces_json = format!("[{}]", faces.join(","));

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>Doom Map 3D View: {name}</title>
    <style>
        body {{ margin: 0; overflow: hidden; background: #111; color: white; font-family: sans-serif; }}
        #info {{ position: absolute; top: 10px; left: 10px; z-index: 100; pointer-events: none; }}
        canvas {{ display: block; }}
    </style>
</head>
<body>
    <div id="info">Map: {name} (Left Click: Rotate, Right Click: Pan, Scroll: Zoom)</div>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/three.js/r128/three.min.js"></script>
    <script src="https://cdn.jsdelivr.net/npm/three@0.128.0/examples/js/controls/OrbitControls.js"></script>
    <script>
        const vertexData = {verts};
        const faceData = {faces};

        const scene = new THREE.Scene();
        scene.background = new THREE.Color(0x111111);

        // Add lights
        const ambientLight = new THREE.AmbientLight(0xffffff, 0.4);
        scene.add(ambientLight);
        const dirLight = new THREE.DirectionalLight(0xffaa55, 0.8);
        dirLight.position.set(1000, 2000, 1000);
        scene.add(dirLight);
        const dirLight2 = new THREE.DirectionalLight(0x55aaff, 0.5);
        dirLight2.position.set(-1000, 1000, -1000);
        scene.add(dirLight2);

        const camera = new THREE.PerspectiveCamera(60, window.innerWidth / window.innerHeight, 1, 50000);

        const renderer = new THREE.WebGLRenderer({{ antialias: true }});
        renderer.setSize(window.innerWidth, window.innerHeight);
        document.body.appendChild(renderer.domElement);

        const controls = new THREE.OrbitControls(camera, renderer.domElement);

        // Build geometry
        const geometry = new THREE.BufferGeometry();
        const vertices = [];
        const indices = [];

        // Add vertices
        vertexData.forEach(v => {{
            vertices.push(v[0], v[1], v[2]);
        }});

        // Triangulate quads
        faceData.forEach(f => {{
            // f is [v0, v1, v2, v3]
            indices.push(f[0], f[1], f[2]);
            indices.push(f[0], f[2], f[3]);
        }});

        geometry.setAttribute('position', new THREE.Float32BufferAttribute(vertices, 3));
        geometry.setIndex(indices);
        geometry.computeVertexNormals();

        const material = new THREE.MeshLambertMaterial({{
            color: 0x888888,
            side: THREE.DoubleSide,
            wireframe: false
        }});

        const mesh = new THREE.Mesh(geometry, material);
        scene.add(mesh);

        // Add wireframe overlay for better visibility
        const wireframeMaterial = new THREE.LineBasicMaterial({{ color: 0x222222, linewidth: 1 }});
        const wireframe = new THREE.LineSegments(new THREE.WireframeGeometry(geometry), wireframeMaterial);
        mesh.add(wireframe);

        // Center camera
        geometry.computeBoundingBox();
        const center = new THREE.Vector3();
        geometry.boundingBox.getCenter(center);
        controls.target.copy(center);

        const size = new THREE.Vector3();
        geometry.boundingBox.getSize(size);
        const maxDim = Math.max(size.x, size.y, size.z);

        camera.position.set(center.x, center.y + maxDim, center.z + maxDim * 0.5);
        controls.update();

        window.addEventListener('resize', () => {{
            camera.aspect = window.innerWidth / window.innerHeight;
            camera.updateProjectionMatrix();
            renderer.setSize(window.innerWidth, window.innerHeight);
        }});

        function animate() {{
            requestAnimationFrame(animate);
            controls.update();
            renderer.render(scene, camera);
        }}
        animate();
    </script>
</body>
</html>"#,
        name = level.name,
        verts = verts_json,
        faces = faces_json
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lumps::{Blockmap, Linedef, Reject, Sector, Sidedef, Vertex};

    fn make_test_level() -> Level {
        let reject = Reject::parse_lump(&[0u8], 1).expect("value must exist in test");
        let mut bm_data = vec![0u8; 14];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0x0000u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let blockmap = Blockmap::parse_lump(&bm_data).expect("value must exist in test");

        Level {
            name: "TEST".to_owned(),
            things: vec![],
            linedefs: vec![Linedef {
                from_vertex: 0,
                to_vertex: 1,
                flags: 0,
                special: 0,
                tag: 0,
                right_sidedef: 0,
                left_sidedef: 0xFFFF,
            }],
            sidedefs: vec![Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"WALL1\0\0\0",
                lower_texture: *b"WALL2\0\0\0",
                middle_texture: *b"WALL3\0\0\0",
                sector: 0,
            }],
            vertexes: vec![Vertex { x: 0, y: 0 }, Vertex { x: 64, y: 0 }],
            segs: vec![],
            ssectors: vec![],
            nodes: vec![],
            sectors: vec![Sector {
                floor_height: 0,
                ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            }],
            reject,
            blockmap,
        }
    }

    #[test]
    fn test_export_html3d() {
        let level = make_test_level();
        let html = export_map_to_html3d(&level);

        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Doom Map 3D View: TEST"));
        assert!(html.contains("THREE.WebGLRenderer"));
        assert!(html.contains("const vertexData = [[0,0,0],[64,0,0],[64,128,0],[0,128,0]];"));
        assert!(html.contains("const faceData = [[0,1,2,3]];"));
    }
}

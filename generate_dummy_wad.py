import struct

with open('dummy.wad', 'wb') as f:
    f.write(b'IWAD') # magic
    f.write(struct.pack('<I', 11)) # num lumps
    f.write(struct.pack('<I', 12)) # dir offset

    # 1 Seg: start_v=0, end_v=0, angle=0, linedef=0, side=0, offset=0 (12 bytes)
    seg_data = struct.pack('<H H h H H h', 0, 0, 0, 0, 0, 0)

    # 1 Vertex: x=0, y=0 (4 bytes)
    vertex_data = struct.pack('<h h', 0, 0)

    # 1 Linedef: v1=0, v2=0, flags=0, types=0, tag=0, rs=0, ls=0xFFFF (14 bytes)
    linedef_data = struct.pack('<H H H H H H H', 0, 0, 0, 0, 0, 0, 0xFFFF)

    # 1 Sidedef: x=0, y=0, u='', l='', m='', sec=0 (30 bytes)
    sidedef_data = struct.pack('<h h 8s 8s 8s H', 0, 0, b'-'*8, b'-'*8, b'-'*8, 0)

    # 1 Sector: floor=0, ceil=100, ftex='-', ctex='-', light=255, type=0, tag=0 (26 bytes)
    sector_data = struct.pack('<h h 8s 8s h H H', 0, 100, b'-'*8, b'-'*8, 255, 0, 0)

    lumps = [
        (b'E1M1\0\0\0\0', b''),
        (b'THINGS\0\0', b''),
        (b'LINEDEFS', linedef_data),
        (b'SIDEDEFS', sidedef_data),
        (b'VERTEXES', vertex_data),
        (b'SEGS\0\0\0\0', seg_data),
        (b'SSECTORS', b'\x01\x00\x00\x00'), # 1 seg, first=0
        (b'NODES\0\0\0', b''),
        (b'SECTORS\0', sector_data),
        (b'REJECT\0\0', b'\x00'), # 1 byte for 1 sector
        (b'BLOCKMAP', b'\x00\x00\x00\x00\x01\x00\x01\x00\x00\x00\x00\x00\xff\xff')
    ]

    offset = 12 + 11 * 16 # Start of lump data
    for name, data in lumps:
        f.write(struct.pack('<I', offset)) # filepos
        f.write(struct.pack('<I', len(data))) # size
        f.write(name)
        offset += len(data)

    for name, data in lumps:
        f.write(data)

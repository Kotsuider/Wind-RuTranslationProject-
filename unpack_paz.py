# -*- coding: utf-8 -*-
import os
import sys
import struct
import zlib

P_INIT = [
    0x243F6A88, 0x85A308D3, 0x13198A2E, 0x03707344, 0xA4093822, 0x299F31D0,
    0x082EFA98, 0xEC4E6C89, 0x452821E6, 0x38D01377, 0xBE5466CF, 0x34E90C6C,
    0xC0AC29B7, 0xC97C50DD, 0x44255072, 0x8E430BFE, 0xF501A271, 0xFBFA3942
]

ARCHIVE_KEYS = {
    'scr': {
        'index': bytes.fromhex('fc0b5121d5310ac692a0c68558b22188accc09b8c66f8753cb83dea4b6a0ae88'),
        'data':  bytes.fromhex('9399648d1d52f4753ba189bda683b3162be8a1051677c7ea001550b3d7e61bdf')
    },
    'bg': {
        'index': bytes.fromhex('a816e5a46d6a3fe3e4130f1c1a361393c918180f0e0f2855fdae00b7f1f09c87'),
        'data':  bytes.fromhex('c18809a0bc4f4dda9b358fc1394f9eca967864c59bf8f1c351ee47aa3f2abd14')
    },
    'st': {
        'index': bytes.fromhex('391e811efc28a244034c86b631a67f7d688da9f0955c7b0aaf82a2d7618de0ca'),
        'data':  bytes.fromhex('b64fa756cbea0a5fb645203066bb65b4fb43f1b3eff8dca4a85e720f3826f15d')
    },
    'sys': {
        'index': bytes.fromhex('16ebc7a4422cedb887827f568e33d0024fd5bd507af89d20da1977ea568749f1'),
        'data':  bytes.fromhex('0a2590b7395396c253793f004b6825805363fbb7628754292df036b8425c3348')
    },
    'bgm': {
        'index': bytes.fromhex('68071b0bbff99cccc6b00dc4dfcf32a006581ae59f456905344df295dfb05f57'),
        'data':  bytes.fromhex('1305d786b57c1e81fa3bb1e7c9b8b89e54b2f6740477e6b0c9edb53a9c0ebf23')
    },
    'voice': {
        'index': bytes.fromhex('16bca1eab2cfebceda900385942a9096d4424224a16579e636b6483dcc99701b'),
        'data':  bytes.fromhex('eba9524aaf188bd8f3b5f7597fccbb3977c81451a35adb56215ed4ea453d8ffb')
    },
    'se': {
        'index': bytes.fromhex('c7143b961bb0d5ca989af5deddb696e0c17a7842e0382bb117af6b08ac95871a'),
        'data':  bytes.fromhex('6789924957968bc545cf568ecc868840f3004b72d08140fa9e0d35dffeaa609d')
    },
    'mov': {
        'index': bytes.fromhex('e2d78f24b95e158f990836d4a9a6b03b95abc1357d609914ce5110dbcfb56ef1'),
        'data':  bytes.fromhex('20dfec879c2a8807b735d279d1873c61bf734cbed9fde03968833cee46d7ad97')
    }
}

def load_s_boxes():
    candidates = ['WindRP_ru.exe', 'WindRP_unpacked.exe', 'WindRP.exe']
    base_dir = os.path.dirname(os.path.abspath(__file__)) if '__file__' in globals() else '.'
    exe_path = None
    for name in candidates:
        for p in [os.path.join(base_dir, name), name]:
            if os.path.isfile(p) and os.path.getsize(p) >= 0x9ad80 + 4096:
                exe_path = p
                break
        if exe_path:
            break
    if not exe_path:
        raise FileNotFoundError("Neither WindRP_ru.exe nor WindRP_unpacked.exe was found")
    with open(exe_path, 'rb') as f:
        f.seek(0x9ad80)
        s_data = f.read(4 * 256 * 4)
    boxes = []
    for s in range(4):
        boxes.append([struct.unpack_from('<I', s_data, (s * 256 + i) * 4)[0] for i in range(256)])
    return boxes

S_INIT = load_s_boxes()

class MusicaBlowfish:
    def __init__(self, raw_key: bytes):
        key = bytes((-b) & 0xff for b in raw_key)
        self.P = list(P_INIT)
        self.S = [list(box) for box in S_INIT]
        key_len = len(key)
        key_pos = 0
        for i in range(18):
            data = 0
            for _ in range(4):
                data = (data << 8) | key[key_pos]
                key_pos = (key_pos + 1) % key_len
            self.P[i] ^= data

        datal = 0
        datar = 0
        for i in range(0, 18, 2):
            datal, datar = self._encipher(datal, datar)
            self.P[i] = datal
            self.P[i+1] = datar

        for s in range(4):
            for i in range(0, 256, 2):
                datal, datar = self._encipher(datal, datar)
                self.S[s][i] = datal
                self.S[s][i+1] = datar

    def _encipher(self, xl, xr):
        P = self.P
        S = self.S
        for i in range(16):
            xl ^= P[i]
            a = (xl >> 24) & 0xff
            b = (xl >> 16) & 0xff
            c = (xl >> 8) & 0xff
            d = xl & 0xff
            f = (((S[0][a] + S[1][b]) & 0xffffffff) ^ S[2][c])
            f = (f + S[3][d]) & 0xffffffff
            xr ^= f
            xl, xr = xr, xl
        xl, xr = xr, xl
        xr ^= P[16]
        xl ^= P[17]
        return xl & 0xffffffff, xr & 0xffffffff

    def decipher_block(self, xl, xr):
        P = self.P
        S = self.S
        for i in range(17, 1, -1):
            xl ^= P[i]
            a = (xl >> 24) & 0xff
            b = (xl >> 16) & 0xff
            c = (xl >> 8) & 0xff
            d = xl & 0xff
            f = (((S[0][a] + S[1][b]) & 0xffffffff) ^ S[2][c])
            f = (f + S[3][d]) & 0xffffffff
            xr ^= f
            xl, xr = xr, xl
        xl, xr = xr, xl
        xr ^= P[1]
        xl ^= P[0]
        return xl & 0xffffffff, xr & 0xffffffff

    def decrypt(self, data: bytes) -> bytearray:
        out = bytearray()
        for i in range(0, len(data), 8):
            w0, w1 = struct.unpack_from('<II', data, i)
            o0, o1 = self.decipher_block(w0, w1)
            out.extend(struct.pack('<II', o0, o1))
        return out

def get_base_type(filename: str):
    name = os.path.basename(filename).lower()
    if name.endswith('.paz'):
        name = name[:-4]
    while name and name[-1].isdigit():
        name = name[:-1]
    return name

def unpack_paz(paz_path: str, output_dir: str = None):
    base_type = get_base_type(paz_path)
    if base_type not in ARCHIVE_KEYS:
        raise ValueError(f'Unknown archive type: {base_type}')

    keys = ARCHIVE_KEYS[base_type]
    bf_index = MusicaBlowfish(keys['index'])
    bf_data = MusicaBlowfish(keys['data'])

    if output_dir is None:
        paz_name = os.path.splitext(os.path.basename(paz_path))[0]
        output_dir = os.path.join(os.path.dirname(paz_path) or '.', f'unpacked_{paz_name}')

    os.makedirs(output_dir, exist_ok=True)
    print(f'[*] Распаковка {paz_path} (тип: {base_type})...')

    with open(paz_path, 'rb') as f:
        hdr = f.read(4)
        index_size = struct.unpack('<I', hdr)[0]
        xor_key = (index_size >> 24) & 0xff
        if xor_key != 0:
            index_size ^= (xor_key << 24 | xor_key << 16 | xor_key << 8 | xor_key)

        raw_index = f.read(index_size)
        if xor_key != 0:
            raw_index = bytes(b ^ xor_key for b in raw_index)

        dec_index = bf_index.decrypt(raw_index)
        count = struct.unpack_from('<I', dec_index, 0)[0]
        print(f'[*] Файлов в индексе: {count}')

        pos = 4
        entries = []
        for _ in range(count):
            null_pos = dec_index.find(b'\x00', pos)
            filename = dec_index[pos:null_pos].decode('cp932', errors='replace')
            pos = null_pos + 1
            offset, unpacked_size, size, aligned_size, is_packed = struct.unpack_from('<QIIII', dec_index, pos)
            pos += 24
            entries.append((filename, offset, unpacked_size, size, aligned_size, is_packed))

        for idx, (fname, offset, unpacked_size, size, aligned_size, is_packed) in enumerate(entries):
            f.seek(offset)
            encrypted_data = f.read(aligned_size)
            if xor_key != 0:
                encrypted_data = bytes(b ^ xor_key for b in encrypted_data)

            decrypted_data = bf_data.decrypt(encrypted_data)[:size]

            if is_packed:
                try:
                    final_data = zlib.decompress(decrypted_data)
                except Exception as e:
                    print(f'[!] Ошибка zlib для {fname}: {e}')
                    final_data = decrypted_data
            else:
                final_data = decrypted_data

            out_file_path = os.path.join(output_dir, fname)
            os.makedirs(os.path.dirname(out_file_path), exist_ok=True)
            with open(out_file_path, 'wb') as out_f:
                out_f.write(final_data)

            if idx < 10 or idx == count - 1:
                print(f'  [{idx+1}/{count}] Извлечен: {fname} ({len(final_data)} байт)')
            elif idx == 10:
                print(f'  ... распаковывается еще {count - 11} файлов ...')

    print(f'[+] Успешно распаковано в: {output_dir}')

if __name__ == '__main__':
    target = sys.argv[1] if len(sys.argv) > 1 else 'scr1.paz'
    unpack_paz(target)

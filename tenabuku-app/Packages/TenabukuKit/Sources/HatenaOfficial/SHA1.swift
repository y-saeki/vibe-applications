// SHA-1 and HMAC-SHA1, which OAuth 1.0a signs with. Written here rather than
// taken from CryptoKit so that the package builds on Linux without a
// dependency. SHA-1 is only used for request signatures, as OAuth 1.0a demands.

enum SHA1 {
    static func hash(_ message: [UInt8]) -> [UInt8] {
        var h0: UInt32 = 0x6745_2301
        var h1: UInt32 = 0xEFCD_AB89
        var h2: UInt32 = 0x98BA_DCFE
        var h3: UInt32 = 0x1032_5476
        var h4: UInt32 = 0xC3D2_E1F0

        var data = message
        let bitLength = UInt64(message.count) * 8
        data.append(0x80)
        while data.count % 64 != 56 {
            data.append(0)
        }
        for shift in stride(from: 56, through: 0, by: -8) {
            data.append(UInt8(truncatingIfNeeded: bitLength >> UInt64(shift)))
        }

        var w = [UInt32](repeating: 0, count: 80)
        for chunk in stride(from: 0, to: data.count, by: 64) {
            for i in 0..<16 {
                let offset = chunk + i * 4
                var word = UInt32(data[offset]) << 24
                word |= UInt32(data[offset + 1]) << 16
                word |= UInt32(data[offset + 2]) << 8
                word |= UInt32(data[offset + 3])
                w[i] = word
            }
            for i in 16..<80 {
                w[i] = rotateLeft(w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16], by: 1)
            }

            var a = h0, b = h1, c = h2, d = h3, e = h4
            for i in 0..<80 {
                let f: UInt32
                let k: UInt32
                switch i {
                case 0..<20:
                    f = (b & c) | (~b & d)
                    k = 0x5A82_7999
                case 20..<40:
                    f = b ^ c ^ d
                    k = 0x6ED9_EBA1
                case 40..<60:
                    f = (b & c) | (b & d) | (c & d)
                    k = 0x8F1B_BCDC
                default:
                    f = b ^ c ^ d
                    k = 0xCA62_C1D6
                }
                let temp = rotateLeft(a, by: 5) &+ f &+ e &+ k &+ w[i]
                e = d
                d = c
                c = rotateLeft(b, by: 30)
                b = a
                a = temp
            }
            h0 = h0 &+ a
            h1 = h1 &+ b
            h2 = h2 &+ c
            h3 = h3 &+ d
            h4 = h4 &+ e
        }

        var digest: [UInt8] = []
        for word in [h0, h1, h2, h3, h4] {
            digest.append(UInt8(truncatingIfNeeded: word >> 24))
            digest.append(UInt8(truncatingIfNeeded: word >> 16))
            digest.append(UInt8(truncatingIfNeeded: word >> 8))
            digest.append(UInt8(truncatingIfNeeded: word))
        }
        return digest
    }

    private static func rotateLeft(_ value: UInt32, by count: UInt32) -> UInt32 {
        (value << count) | (value >> (32 - count))
    }
}

enum HMACSHA1 {
    private static let blockSize = 64

    static func authenticate(_ message: [UInt8], key: [UInt8]) -> [UInt8] {
        var key = key.count > blockSize ? SHA1.hash(key) : key
        key += [UInt8](repeating: 0, count: blockSize - key.count)
        let inner = key.map { $0 ^ 0x36 }
        let outer = key.map { $0 ^ 0x5C }
        return SHA1.hash(outer + SHA1.hash(inner + message))
    }
}

/**
 * This file is part of radio_tool.
 * Copyright (c) 2026 v0l <radio_tool@v0l.io>
 *
 * radio_tool is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * radio_tool is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with radio_tool. If not, see <https://www.gnu.org/licenses/>.
 */
#pragma once

#include <cstdint>
#include <cstring>
#include <fstream>
#include <iomanip>
#include <sstream>
#include <string>

namespace radio_tool
{
	/**
	 * FIPS 180-4 SHA-256, enough of it to put a hash in the flash
	 * confirmation without pulling in a crypto dependency
	 */
	class SHA256
	{
	public:
		SHA256() { Reset(); }

		auto Reset() -> void
		{
			state[0] = 0x6a09e667;
			state[1] = 0xbb67ae85;
			state[2] = 0x3c6ef372;
			state[3] = 0xa54ff53a;
			state[4] = 0x510e527f;
			state[5] = 0x9b05688c;
			state[6] = 0x1f83d9ab;
			state[7] = 0x5be0cd19;
			bits = 0;
			buflen = 0;
		}

		auto Update(const uint8_t *data, size_t size) -> void
		{
			bits += (uint64_t)size * 8;
			while (size > 0)
			{
				const size_t take = std::min(size, sizeof(buf) - buflen);
				std::memcpy(buf + buflen, data, take);
				buflen += take;
				data += take;
				size -= take;
				if (buflen == sizeof(buf))
				{
					Compress(buf);
					buflen = 0;
				}
			}
		}

		auto Digest() -> std::string
		{
			const uint64_t total_bits = bits;
			const uint8_t pad = 0x80;
			Update(&pad, 1);
			const uint8_t zero = 0x00;
			while (buflen != 56)
			{
				Update(&zero, 1);
			}
			const uint8_t tail[8] = {
				(uint8_t)(total_bits >> 56), (uint8_t)(total_bits >> 48),
				(uint8_t)(total_bits >> 40), (uint8_t)(total_bits >> 32),
				(uint8_t)(total_bits >> 24), (uint8_t)(total_bits >> 16),
				(uint8_t)(total_bits >> 8), (uint8_t)(total_bits)};
			Update(tail, sizeof(tail));

			std::ostringstream out;
			for (const auto &w : state)
			{
				out << std::hex << std::setfill('0') << std::setw(8) << w;
			}
			Reset();
			return out.str();
		}

		/**
		 * Hash a file by streaming it, so the size of a firmware image does
		 * not matter. Throws when the file cannot be opened.
		 */
		static auto File(const std::string &path) -> std::string
		{
			std::ifstream in(path, std::ios_base::binary);
			if (!in.is_open())
			{
				throw std::runtime_error("Cant open file to hash: " + path);
			}

			SHA256 h;
			char chunk[65536];
			while (in.good())
			{
				in.read(chunk, sizeof(chunk));
				h.Update((const uint8_t *)chunk, (size_t)in.gcount());
			}
			return h.Digest();
		}

	private:
		static auto Rotr(const uint32_t x, const uint32_t n) -> uint32_t
		{
			return (x >> n) | (x << (32 - n));
		}

		auto Compress(const uint8_t block[64]) -> void
		{
			static const uint32_t k[64] = {
				0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
				0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
				0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
				0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
				0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
				0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
				0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
				0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2};

			uint32_t w[64];
			for (auto i = 0u; i < 16; i++)
			{
				w[i] = ((uint32_t)block[i * 4] << 24) | ((uint32_t)block[i * 4 + 1] << 16) |
					   ((uint32_t)block[i * 4 + 2] << 8) | (uint32_t)block[i * 4 + 3];
			}
			for (auto i = 16u; i < 64; i++)
			{
				const auto s0 = Rotr(w[i - 15], 7) ^ Rotr(w[i - 15], 18) ^ (w[i - 15] >> 3);
				const auto s1 = Rotr(w[i - 2], 17) ^ Rotr(w[i - 2], 19) ^ (w[i - 2] >> 10);
				w[i] = w[i - 16] + s0 + w[i - 7] + s1;
			}

			auto a = state[0], b = state[1], c = state[2], d = state[3];
			auto e = state[4], f = state[5], g = state[6], h = state[7];
			for (auto i = 0u; i < 64; i++)
			{
				const auto S1 = Rotr(e, 6) ^ Rotr(e, 11) ^ Rotr(e, 25);
				const auto ch = (e & f) ^ (~e & g);
				const auto t1 = h + S1 + ch + k[i] + w[i];
				const auto S0 = Rotr(a, 2) ^ Rotr(a, 13) ^ Rotr(a, 22);
				const auto maj = (a & b) ^ (a & c) ^ (b & c);
				const auto t2 = S0 + maj;
				h = g;
				g = f;
				f = e;
				e = d + t1;
				d = c;
				c = b;
				b = a;
				a = t1 + t2;
			}

			state[0] += a; state[1] += b; state[2] += c; state[3] += d;
			state[4] += e; state[5] += f; state[6] += g; state[7] += h;
		}

		uint32_t state[8];
		uint64_t bits;
		uint8_t buf[64];
		size_t buflen;
	};
} // namespace radio_tool

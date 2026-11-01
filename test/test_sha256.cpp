/**
 * This file is part of radio_tool.
 * Copyright (c) 2026 v0l <radio_tool@v0l.io>
 *
 * radio_tool is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 */
#include <radio_tool/util/sha256.hpp>

#include <fstream>
#include <iostream>
#include <string>
#include <vector>

using radio_tool::SHA256;

namespace
{
	struct Vector
	{
		std::string input;
		std::string digest;
	};

	const std::vector<Vector> Vectors = {
		{"", "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"},
		{"abc", "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"},
		{"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
		 "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"},
		{"The quick brown fox jumps over the lazy dog",
		 "d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592"}};
}

int main()
{
	for (const auto &v : Vectors)
	{
		SHA256 h;
		h.Update((const uint8_t *)v.input.data(), v.input.size());
		if (h.Digest() != v.digest)
		{
			std::cerr << "FAIL: digest mismatch for input of " << v.input.size() << " bytes" << std::endl;
			return 1;
		}

		// the same bytes fed one at a time must land in the same place
		SHA256 slow;
		for (const auto c : v.input)
		{
			slow.Update((const uint8_t *)&c, 1);
		}
		if (slow.Digest() != v.digest)
		{
			std::cerr << "FAIL: byte-wise update mismatch" << std::endl;
			return 1;
		}
	}

	// File() must agree with the in-memory path
	{
		const char *name = "/tmp/radio_tool_sha256_test.bin";
		std::ofstream out(name, std::ios_base::binary);
		out << "abc";
		out.close();

		SHA256 h;
		h.Update((const uint8_t *)"abc", 3);
		if (SHA256::File(name) != h.Digest())
		{
			std::cerr << "FAIL: file hash mismatch" << std::endl;
			return 1;
		}
	}

	std::cout << "sha256 ok" << std::endl;
	return 0;
}

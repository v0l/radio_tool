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

#include <string>
#include <vector>
#include <cstdint>

namespace radio_tool::device
{
	/**
	 * A bidirectional stream of bytes to a radio. Some radios clone over a
	 * serial cable and some over Bluetooth LE, but the clone protocol on top
	 * is identical, so the radio drivers work against this interface.
	 */
	class ByteStream
	{
	public:
		virtual ~ByteStream() = default;

		/**
		 * Write all bytes to the radio
		 */
		virtual auto Write(const std::vector<uint8_t> &data) const -> void = 0;

		/**
		 * Read up to len bytes, returns fewer bytes if the radio stops talking
		 */
		virtual auto Read(const size_t &len, const uint32_t &timeout_ms) const -> std::vector<uint8_t> = 0;

		/**
		 * Discard anything already received but not yet read
		 */
		virtual auto FlushInput() const -> void = 0;

		/**
		 * Read exactly len bytes or throw
		 */
		auto ReadExact(const size_t &len, const uint32_t &timeout_ms, const std::string &what) const -> std::vector<uint8_t>;
	};
} // namespace radio_tool::device

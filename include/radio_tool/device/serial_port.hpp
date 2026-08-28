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

#include <radio_tool/device/byte_stream.hpp>

#include <string>
#include <vector>
#include <cstdint>

namespace radio_tool::device
{
	/**
	 * A plain 8N1 serial port, for radios which clone over a serial cable
	 * rather than USB (Baofeng UV-5R and friends)
	 */
	class SerialPort : public ByteStream
	{
	public:
		SerialPort(const std::string &port, const uint32_t &baud);
		~SerialPort();

		SerialPort(const SerialPort &) = delete;
		auto operator=(const SerialPort &) -> SerialPort & = delete;

		/**
		 * Write all bytes to the port
		 */
		auto Write(const std::vector<uint8_t> &data) const -> void override;

		/**
		 * Write bytes one at a time with a delay between each byte,
		 * some radios drop the magic if it arrives in a single burst
		 */
		auto WriteSlow(const std::vector<uint8_t> &data, const uint32_t &delay_ms) const -> void;

		/**
		 * Read up to len bytes, returns fewer bytes if the port times out
		 */
		auto Read(const size_t &len, const uint32_t &timeout_ms) const -> std::vector<uint8_t> override;

		/**
		 * Discard anything sitting in the input buffer
		 */
		auto FlushInput() const -> void override;

		static auto Sleep(const uint32_t &ms) -> void;

	private:
		const std::string port;
#ifdef _WIN32
		void *handle;
#else
		int fd;
#endif
	};
} // namespace radio_tool::device

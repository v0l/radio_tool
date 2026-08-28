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

#include <radio_tool/radio/radio.hpp>
#include <radio_tool/device/serial_port.hpp>

#include <memory>
#include <string>
#include <vector>

namespace radio_tool::radio
{
	/**
	 * Clone mode (codeplug) support for the Baofeng UV-5R family.
	 *
	 * The protocol is the one implemented by CHIRP's uv5r driver:
	 * https://github.com/kk7ds/chirp/blob/master/chirp/drivers/uv5r.py
	 *
	 * The file this produces is byte compatible with a CHIRP .img for the
	 * same model: an 8 byte ident block, the main memory (0x0000-0x1800)
	 * and, on radios which have it, the aux memory (0x1EC0-0x2000).
	 */
	class UV5RRadio : public RadioOperations
	{
	public:
		static constexpr auto BaudRate = 9600u;

		/**
		 * Identify strings sent to the radio to start a clone session
		 */
		struct UV5RModel
		{
			const char *name;
			std::vector<std::vector<uint8_t>> idents;
			bool aux_block;
		};

		UV5RRadio(const std::string &port, const UV5RModel &model);

		auto WriteFirmware(const std::string &file) -> void override;
		auto ReadCodeplug(const std::string &file) -> void override;
		auto ToString() const -> const std::string override;

		/**
		 * All models this driver speaks to, keyed by the value of --radio
		 */
		static auto Models() -> const std::vector<UV5RModel> &;

		/**
		 * Look up a model by name, returns nullptr when unknown
		 */
		static auto GetModel(const std::string &name) -> const UV5RModel *;

		/**
		 * Download the codeplug image without writing it to disk
		 */
		auto Download() const -> std::vector<uint8_t>;

	private:
		auto Identify() const -> void;
		auto TryIdent(const std::vector<uint8_t> &magic) const -> std::vector<uint8_t>;
		auto ReadBlock(const uint16_t &addr, const uint8_t &size, const bool &first) const -> std::vector<uint8_t>;
		auto ReadFirmwareVersion() const -> void;

		device::SerialPort port;
		const UV5RModel model;

		mutable std::vector<uint8_t> ident;
		mutable std::string firmware_version;
		mutable bool dropped_byte = false;
	};
} // namespace radio_tool::radio

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
#include <radio_tool/device/byte_stream.hpp>

#include <memory>
#include <string>
#include <vector>

namespace radio_tool::radio
{
	/**
	 * Codeplug support for the Baofeng UV-17Pro family, which despite the
	 * name includes the UV-5R Mini (a completely different radio to the
	 * classic UV-5R, see uv5r_radio.hpp).
	 *
	 * Protocol as implemented by CHIRP's baofeng_uv17Pro driver:
	 * https://github.com/kk7ds/chirp/blob/master/chirp/drivers/baofeng_uv17Pro.py
	 *
	 * These radios talk the same clone protocol over a cable and over
	 * Bluetooth LE, so they work with either --port or --ble.
	 */
	class UV17ProRadio : public RadioOperations
	{
	public:
		static constexpr auto BaudRate = 115200u;
		static constexpr auto BlockSize = 0x40u;

		/**
		 * A contiguous region of radio memory
		 */
		struct MemoryRegion
		{
			uint16_t start;
			uint16_t size;
		};

		struct UV17ProModel
		{
			const char *name;
			const char *ident;					   //magic string which starts a clone session
			const char *chirp_model;			   //model name CHIRP stamps on an image
			std::vector<MemoryRegion> regions;
			uint8_t encryption_key;				   //index into the obfuscation table
		};

		UV17ProRadio(const std::string &port, const UV17ProModel &model);
		UV17ProRadio(std::unique_ptr<device::ByteStream> stream, const UV17ProModel &model);

		auto WriteFirmware(const std::string &file) -> void override;
		auto ReadCodeplug(const std::string &file) -> void override;
		auto ToString() const -> const std::string override;

		static auto Models() -> const std::vector<UV17ProModel> &;
		static auto GetModel(const std::string &name) -> const UV17ProModel *;

		/**
		 * Download the codeplug image without writing it to disk
		 */
		auto Download() const -> std::vector<uint8_t>;

		/**
		 * The "encryption" these radios apply to memory reads, which is a
		 * conditional xor against a 4 byte key. Symmetric, so the same call
		 * encrypts and decrypts.
		 */
		static auto Crypt(const uint8_t &key_index, const std::vector<uint8_t> &data) -> std::vector<uint8_t>;

	private:
		auto Identify() const -> void;
		auto SendMagic(const std::vector<uint8_t> &magic, const size_t &response_len, const std::string &what) const -> std::vector<uint8_t>;
		auto ReadBlock(const uint16_t &addr, const uint8_t &size) const -> std::vector<uint8_t>;

		std::unique_ptr<device::ByteStream> port;
		const UV17ProModel model;

		mutable std::vector<uint8_t> ident_response;
	};
} // namespace radio_tool::radio

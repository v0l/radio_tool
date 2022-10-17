/**
 * This file is part of radio_tool.
 * Copyright (c) 2020 v0l <radio_tool@v0l.io>
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

#include <radio_tool/fw/fw.hpp>
#include <radio_tool/fw/cipher/sgl.hpp>

#include <fstream>
#include <cstring>
#include <sstream>
#include <iomanip>

namespace radio_tool::fw
{
	constexpr auto NotAscii = [](unsigned char ch) {
		return !(ch >= ' ' && ch <= '~');
	};

	class SGLHeader
	{
	public:
		SGLHeader(
			const uint16_t& sgl_version,
			const uint32_t& len,
			const std::string& group,
			const std::string& model,
			const std::string& version,
			const std::string& key,
			const uint8_t& binary_offset,
			const uint16_t& header2_offset)
			: sgl_version(sgl_version),
			length(len),
			radio_group(group.begin(), std::find_if(group.begin(), group.end(), NotAscii)),
			radio_model(model.begin(), std::find_if(model.begin(), model.end(), NotAscii)),
			protocol_version(version),
			model_key(key),
			binary_offset(binary_offset),
			header2_offset(header2_offset)
		{
			if (binary_offset >= 0x81) {
				throw std::runtime_error("Binary offset must be < 0x81");
			}
			if (header2_offset < 0x1f || header2_offset > 0x101) {
				throw std::runtime_error("Header 2 offset must be greater than 0x1f and less than 0x101");
			}
		}

		auto ToString() const->std::string;

		auto Serialize(bool encrypt = true) const->std::vector<uint8_t>;

		const uint16_t sgl_version;
		const uint32_t length;
		const uint8_t binary_offset;
		const uint16_t header2_offset;
		const std::string radio_group; //BF-DMR = 0x10
		const std::string radio_model; //1801 = 0x08
		const std::string protocol_version; //V1.00.1 = 0x08
		const std::string model_key; //DV01xxxx = 0x08
	};

	/**
	 * Class to store all config for each TYT radio model (SGL firmware)
	 */
	class TYTSGLRadioConfig
	{
	public:
		TYTSGLRadioConfig(const std::string& model, const SGLHeader& header, const uint8_t* cipher, const uint32_t& cipher_l, const uint16_t& xor_offset)
			: radio_model(model), header(header), cipher(cipher), cipher_len(cipher_l), xor_offset(xor_offset)
		{
		}

		/**
		 * The model of the radio
		 */
		const std::string radio_model;

		/**
		 * Decrypted firmware file header
		 */
		const SGLHeader header;

		/**
		 * The cipher key for encrypting/decrypting the firmware
		 */
		const uint8_t* cipher;

		/**
		 * The length of the cipher
		 */
		const uint32_t cipher_len;

		/**
		 * Offset into xor key
		 */
		const uint16_t xor_offset;
	};

	namespace tyt::config::sgl
	{
		const std::vector<uint8_t> Magic = { 'S', 'G', 'L', '!' };

		const std::vector<TYTSGLRadioConfig> All = {
			TYTSGLRadioConfig("GD77", SGLHeader(1, 0, "SG-MD-760", "MD-760", "V1.00.01", "DV01xxx", 0x00, 0xff), fw::cipher::sgl, fw::cipher::sgl_length, 0x807),
			TYTSGLRadioConfig("GD77S", SGLHeader(1, 0, "SG-MD-730", "MD-730", "V1.00.01", "DV02xxx", 0x00, 0xff), fw::cipher::sgl, fw::cipher::sgl_length, 0x2a8e),
			TYTSGLRadioConfig("BF5R", SGLHeader(1, 0, "BF-5R", "BF-5R", "V1.00.01", "DV02xxx", 0x00, 0xff), fw::cipher::sgl, fw::cipher::sgl_length, 0x306e),
			TYTSGLRadioConfig("DM1801", SGLHeader(1, 0, "BF-DMR", "1801", "V1.00.01", "DV03xxx", 0x00, 0xff), fw::cipher::sgl, fw::cipher::sgl_length, 0x2c7c),
		};
	}

	class TYTSGLFW : public FirmwareSupport
	{
	public:
		TYTSGLFW() : FirmwareSupport(), config(nullptr) {}

		auto Read(const std::string& file) -> void override;
		auto Write(const std::string& file) -> void override;
		auto ToString() const->std::string override;
		auto Decrypt() -> void override;
		auto Encrypt() -> void override;
		auto SetRadioModel(const std::string&) -> void override;

		auto GetConfig() const -> const TYTSGLRadioConfig* {
			return config;
		}

		/**
		 * @note This is not the "firmware_model" which exists in the firmware header
		 */
		auto GetRadioModel() const -> const std::string override;

		/**
		 * Tests a file if its a valid firmware file
		 */
		static auto SupportsFirmwareFile(const std::string& file) -> bool;

		/**
		 * Tests if a radio model is supported by this firmware handler
		 */
		static auto SupportsRadioModel(const std::string& model) -> bool;

		/**
		 * Test the file if it matches any known headers
		 */
		static auto ReadHeader(const std::string& file) -> const SGLHeader;

		/**
		 * Create an instance of this class for the firmware factory
		 */
		static auto Create() -> std::unique_ptr<FirmwareSupport>
		{
			return std::make_unique<TYTSGLFW>();
		}

	private:
		const TYTSGLRadioConfig* config;
	};

} // namespace radio_tool::fw
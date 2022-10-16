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
	class SGLHeader
	{
	public:
		SGLHeader()
			: sgl_version(1), length(0), radio_group(), radio_model(), protocol_version(), model_key(), binary_offset(0)
		{
		}

		SGLHeader(const std::string& group, const std::string& model, const std::string& version = "V1.00.01", const std::string& key = "DV01xxxx")
			: sgl_version(1), length(0), radio_group(group.begin(), group.end()), radio_model(model.begin(), model.end()), protocol_version(version.begin(), version.end()), model_key(), binary_offset(0)
		{
		}

		SGLHeader(const std::vector<uint8_t>& group, const std::vector<uint8_t>& model)
			: sgl_version(1), length(0), radio_group(group), radio_model(model), protocol_version(), model_key(), binary_offset(0)
		{
		}

		SGLHeader(const uint16_t& sgl, const uint32_t& length, const std::vector<uint8_t>& group, const std::vector<uint8_t>& model, const std::vector<uint8_t>& version, const std::vector<uint8_t>& key, const uint8_t& binary_offset)
			: sgl_version(sgl), length(length), radio_group(group), radio_model(model), protocol_version(version), model_key(key), binary_offset(binary_offset)
		{
		}

		auto Model() const -> const std::string&
		{
			return std::string(radio_model.begin(), radio_model.begin() + 4);
		}

		auto ToString() const->std::string;

		auto Serialize(bool encrypt = true) const->std::vector<uint8_t>;

		const uint16_t sgl_version;
		const uint32_t length;
		const uint8_t binary_offset;
		const std::vector<uint8_t> radio_group; //BF-DMR = 0x10
		const std::vector<uint8_t> radio_model; //1801 = 0x08
		const std::vector<uint8_t> protocol_version; //V1.00.1 = 0x08
		const std::vector<uint8_t> model_key; //DV01xxxx = 0x08
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

		/**
		 * And empty config instance for reference
		 */
		static auto Empty() -> const TYTSGLRadioConfig
		{
			return TYTSGLRadioConfig();
		}
	private:
		TYTSGLRadioConfig()
			: radio_model(), header(), cipher(nullptr), cipher_len(0), xor_offset(0)
		{
		}
	};

	namespace tyt::config::sgl
	{
		const std::vector<uint8_t> Magic = { 'S', 'G', 'L', '!' };

		const std::vector<TYTSGLRadioConfig> All = {
			TYTSGLRadioConfig("GD77", SGLHeader("SG-MD-760", "MD-760", "V1.00.01", "DV01xxx"), fw::cipher::sgl, fw::cipher::sgl_length, 0x807),
			TYTSGLRadioConfig("GD77S", SGLHeader("SG-MD-730", "MD-730", "V1.00.01", "DV02xxx"), fw::cipher::sgl, fw::cipher::sgl_length, 0x2a8e),
			TYTSGLRadioConfig("DM1801", SGLHeader("BF-DMR", "1801", "V1.00.01", "DV03xxx"), fw::cipher::sgl, fw::cipher::sgl_length, 0x2c7c),
			TYTSGLRadioConfig("RD5R", SGLHeader("RD-5R", "RD-5R", "V1.00.01", "DV02xxx"), fw::cipher::sgl, fw::cipher::sgl_length, 0x306e)
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
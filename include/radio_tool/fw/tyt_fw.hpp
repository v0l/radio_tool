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
#include <radio_tool/fw/cipher/uv3x0.hpp>
#include <radio_tool/fw/cipher/dm1701.hpp>
#include <radio_tool/fw/cipher/md380.hpp>
#include <radio_tool/fw/cipher/md9600.hpp>

#include <fstream>
#include <cstring>
#include <sstream>
#include <memory>
#include <iomanip>

namespace radio_tool::fw
{
	/**
	 * Class to store all config for each TYT radio model
	 */
	class TYTRadioConfig
	{
	public:
		TYTRadioConfig(const std::string& model, const std::string& fw_model, const std::vector<uint8_t>& c_magic, const uint8_t* cipher, const uint32_t& cipher_l)
			: radio_model(model), firmware_model(fw_model), counter_magic(c_magic), cipher(cipher), cipher_len(cipher_l)
		{
		}

		/**
		 * The model of the radio
		 */
		const std::string radio_model;

		/**
		 * The model in the firmware file
		 */
		const std::string firmware_model;

		/**
		 * The magic counter value for this radio
		 */
		const std::vector<uint8_t> counter_magic;

		/**
		 * The cipher key for encrypting/decrypting the firmwar
		 */
		const uint8_t* cipher;

		/**
		 * The length of the cipher
		 */
		const uint32_t cipher_len;
	};

	namespace tyt::magic
	{
		using namespace std::literals::string_literals;

		//OutSecurityBin\0\0
		const std::vector<uint8_t> begin = { 0x4f, 0x75, 0x74, 0x53, 0x65, 0x63, 0x75, 0x72, 0x69, 0x74, 0x79, 0x42, 0x69, 0x6e, 0x00, 0x00 };
		//OutSecurityBinEnd
		const std::vector<uint8_t> end = { 0x4f, 0x75, 0x74, 0x70, 0x75, 0x74, 0x42, 0x69, 0x6e, 0x44, 0x61, 0x74, 0x61, 0x45, 0x6e, 0x64 };

		/*
		 * +GPS = Both GPS and Non-GPS versions have the same magic value
		 * CSV = DMR Database upload as CSV support
		 * REC = Recording
		 */
		const std::vector<uint8_t> MD2017_D = { 0x02, 0x19, 0x0c }; //MD-2017 (REC)
		const std::vector<uint8_t> MD2017_S = { 0x02, 0x18, 0x0c }; //MD-2017 GPS (REC)
		const std::vector<uint8_t> MD2017_V = { 0x01, 0x19 };       //MD-2017 (CSV)
		const std::vector<uint8_t> MD2017_P = { 0x01, 0x18 };       //MD-2017 GPS (CSV)

		const std::vector<uint8_t> MD9600 = { 0x01, 0x14 }; //MD-9600 (REC/CSV) +GPS

		const std::vector<uint8_t> UV3X0_GPS = { 0x02, 0x16, 0x0c }; //MD-UV3X0 (REC/CSV)(GPS) / RT3S
		const std::vector<uint8_t> UV3X0 = { 0x02, 0x17, 0x0c };     //MD-UV3X0 (REC/CSV) / RT3S

		const std::vector<uint8_t> DM1701 = { 0x01, 0x0f }; //DM-1701

		const std::vector<uint8_t> MD390 = { 0x01, 0x10 }; //MD-390
		const std::vector<uint8_t> MD380 = { 0x01, 0x0d }; //MD-380 / MD-446
		const std::vector<uint8_t> MD280 = { 0x01, 0x1b }; //MD-280
	} // namespace tyt::magic

	namespace tyt::config
	{
		const std::vector<TYTRadioConfig> All = {
			TYTRadioConfig("MD2017" /* REC */, "MD-9600", tyt::magic::MD2017_D, cipher::uv3x0, cipher::uv3x0_length),
			TYTRadioConfig("MD2017 GPS" /* REC */, "MD-9600", tyt::magic::MD2017_S, cipher::uv3x0, cipher::uv3x0_length),
			TYTRadioConfig("MD2017" /* CSV */, "MD-9600", tyt::magic::MD2017_V, cipher::uv3x0, cipher::uv3x0_length),
			TYTRadioConfig("MD2017 GPS" /* CSV */, "MD-9600", tyt::magic::MD2017_P, cipher::uv3x0, cipher::uv3x0_length),
			TYTRadioConfig("MD9600", "MD-9600", tyt::magic::MD9600, cipher::md9600, cipher::md9600_length),
			TYTRadioConfig("UV3X0 GPS", "MD-9600", tyt::magic::UV3X0_GPS, cipher::uv3x0, cipher::uv3x0_length),
			TYTRadioConfig("UV3X0", "MD-9600", tyt::magic::UV3X0, cipher::uv3x0, cipher::uv3x0_length),
			TYTRadioConfig("DM1701", "DM1701", tyt::magic::DM1701, cipher::dm1701, cipher::dm1701_length),
			TYTRadioConfig("MD390", "JST51", tyt::magic::MD390, cipher::md380, cipher::md380_length),
			TYTRadioConfig("MD380", "JST51", tyt::magic::MD380, cipher::md380, cipher::md380_length),
			TYTRadioConfig("MD446", "JST51", tyt::magic::MD380, cipher::md380, cipher::md380_length),
			TYTRadioConfig("MD280", "JST51", tyt::magic::MD280, cipher::md380, cipher::md380_length)
		};
	}
	/**
	 * Stores the start of the TYT Firmware file header
	 */
	typedef struct
	{
		uint8_t magic[16];
		uint8_t radio[16];
		uint32_t n1, n2, n3, n4;
		uint8_t counter_magic[76];
		uint32_t n_regions;
	} TYTFirmwareHeader;
	static_assert(sizeof(TYTFirmwareHeader) == 128);

	class TYTFW : public FirmwareSupport
	{
	public:
		TYTFW() {}
		TYTFW(const std::vector<uint8_t>& cMagic)
			: FirmwareSupport(0x200), counterMagic(cMagic)
		{ }

		auto Read(const std::string& file) -> void override;
		auto Write(const std::string& file) -> void override;
		auto ToString() const->std::string override;
		auto Decrypt() -> void override;
		auto Encrypt() -> void override;
		auto SetRadioModel(const std::string&) -> void override;

		/**
		 * @note This is not the "firmware_model" which exists in the firmware header
		 */
		auto GetRadioModel() const -> const std::string override;

		/**
		 * Get the counter magic for a specific model radio
		 * @note This is the radio model not the model from the firmware file
		 */
		static auto GetCounterMagic(const std::string& radio) -> const std::vector<uint8_t>
		{
			for (const auto& r : tyt::config::All)
			{
				if (r.radio_model == radio)
				{
					return r.counter_magic;
				}
			}
			throw std::runtime_error("Radio not supported");
		}

		/**
		 * Return the radio model of a specific counter magic sequence
		 * @note This is probably not accurate unless you already know its a TYT firmware file
		 */
		static auto GetRadioFromMagic(const std::vector<uint8_t>& cm) -> const std::string
		{
			for (const auto& r : tyt::config::All)
			{
				if (r.counter_magic.size() != cm.size())
					continue;
				/* GCC doesnt seem to mind which is longer, MSVC tries to read past the end of [first2] */
				if (std::equal(cm.begin(), cm.end(), r.counter_magic.begin()))
				{
					return r.radio_model;
				}
			}
			throw std::runtime_error("Radio not supported");
		}

		/**
		 * Tests a file if its a valid firmware file
		 */
		static auto SupportsFirmwareFile(const std::string& file) -> bool;

		/**
		 * Tests if a radio model is supported by this firmware handler
		 */
		static auto SupportsRadioModel(const std::string& model) -> bool;

		/**
		 * Create an instance of this class for the firmware factory
		 */
		static auto Create() -> std::unique_ptr<FirmwareSupport>
		{
			return std::make_unique<TYTFW>();
		}

	private:
		std::vector<uint8_t> counterMagic; //2-3 bytes
		std::string firmware_model, radio_model;

		static auto ReadHeader(std::ifstream&)->TYTFirmwareHeader;
		static auto CheckHeader(const TYTFirmwareHeader&) -> void;
		auto ApplyXOR() -> void;
	};

} // namespace radio_tool::fw

/**
 * This file is part of radio_tool.
 * Copyright (c) 2022 Niccol� Izzo IU2KIN
 * Copyright (c) 2022 v0l <radio_tool@v0l.io>
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
#include <radio_tool/fw/ailunce_fw.hpp>
#include <radio_tool/util.hpp>

#include <iomanip>

using namespace radio_tool::fw;

auto AilunceFW::Read(const std::string &file) -> void
{
	auto i = std::ifstream(file, std::ios_base::binary);
	if (i.is_open())
	{
		i.seekg(0, std::ios_base::end);
		auto binarySize = i.tellg();
		memory_ranges.push_back(std::make_pair(0, binarySize));
		i.seekg(std::ios_base::beg);
		data.resize(binarySize);
		i.read((char *)data.data(), data.size());
	}
	i.close();
}

auto AilunceFW::Write(const std::string &file) -> void
{
	std::ofstream fout(file, std::ios_base::binary);
	if (fout.is_open())
	{
		fout.write((char *)data.data(), data.size());
		fout.close();
	}
}

auto AilunceFW::ToString() const -> std::string
{
	std::stringstream out;
	out << "== Ailunce Firmware == " << std::endl;
	auto n = 0;
	for (const auto &m : memory_ranges)
	{
		out << "  " << n++ << ": Start=0x" << std::setfill('0') << std::setw(8) << std::hex << m.first
			<< ", Length=0x" << std::setfill('0') << std::setw(8) << std::hex << m.second
			<< std::endl;
	}
	return out.str();
}

auto AilunceFW::SupportsFirmwareFile(const std::string &file) -> bool
{
	std::ifstream i;
	i.open(file, i.binary);
	if (i.is_open())
	{
		i.close();
		return true;
	}
	else
	{
		throw std::runtime_error("Can't open firmware file");
	}
}

// Ailunce hts are flashed with a USB serial adapter, no way to identify
auto AilunceFW::SupportsRadioModel(const std::string &model) -> bool
{
	return model == "HD1";
}

auto AilunceFW::GetRadioModel() const -> const std::string
{
	return "Ailunce HD1";
}

auto AilunceFW::SetRadioModel(const std::string &model) -> void
{
}

auto AilunceFW::Decrypt() -> void
{
	ApplyXOR();
}

auto AilunceFW::Encrypt() -> void
{
	ApplyXOR();
}

auto AilunceFW::ApplyXOR() -> void
{
	for (uint32_t i = 0; i < (data.size() / sizeof(uint32_t)); i++)
	{
		uint32_t *word = reinterpret_cast<uint32_t *>(data.data()) + i;
		if (*word == 0x0 || *word == 0xffffffff)
			*word ^= 0xffffffff;
		else if (*word & (1 << 28))
			*word ^= 0x01111111;
		else
			*word ^= 0x07777777;
	}
	// Last bytes
	for (auto z = data.size() - (data.size() % sizeof(uint32_t));
		 z < data.size(); z++)
	{
		if (data[z] == 0x00 || data[z] == 0xff)
			data[z] ^= 0xff;
		else if (data[z] & 1)
			data[z] ^= 0x01;
		else
			data[z] ^= 0x07;
	}
}

auto AilunceFW::IsCompatible(const FirmwareSupport* Other) const -> bool
{
	if (typeid(Other) != typeid(this)) {
		return false;
	}

	auto afw = dynamic_cast<const AilunceFW*>(Other);
	return afw->radio_model == radio_model;
}
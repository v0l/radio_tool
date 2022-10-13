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
#include <radio_tool/fw/yaesu_fw.hpp>
#include <radio_tool/util.hpp>

using namespace radio_tool::fw;

auto YaesuFW::Read(const std::string& file) -> void
{
	auto i = std::ifstream(file, std::ios_base::binary);
	if (i.is_open())
	{
        // Compute binary file size
		i.seekg(0, std::ios_base::end);
        auto binarySize = i.tellg();
		i.seekg(0);

        // Read binary
		data.resize(binarySize);
		i.read((char*)data.data(), binarySize);
	}
	i.close();
}

auto YaesuFW::Write(const std::string& file) -> void
{
	std::ofstream fout(file, std::ios_base::binary);
	if (fout.is_open())
	{
		//write firmware data
		fout.write((char*)data.data(), data.size());
		fout.close();
	}
}

auto YaesuFW::ToString() const -> std::string
{
	std::stringstream out;
	out << "== Yaesu Firmware == " << std::endl
		<< "Size:  " << std::fixed << std::setprecision(2)
        << (data.size() / 1024.0) << " KiB" << std::endl;
	return out.str();
}

auto YaesuFW::Decrypt() -> void { }
auto YaesuFW::Encrypt() -> void { }
auto YaesuFW::SetRadioModel(const std::string&) -> void { }
auto YaesuFW::GetRadioModel() const -> const std::string { }

auto YaesuFW::SupportsFirmwareFile(const std::string& file) -> bool
{
	std::ifstream i;
	i.open(file, i.binary);
	if (i.is_open())
	{
        return true;
	}
	else
	{
		throw std::runtime_error("Can't open firmware file");
	}
}

auto YaesuFW::SupportsRadioModel(const std::string& model) -> bool
{
    return true;
}

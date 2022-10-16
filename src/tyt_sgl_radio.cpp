/**
 * This file is part of radio_tool.
 * Copyright (c) 2021 v0l <radio_tool@v0l.io>
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
#include <radio_tool/radio/tyt_sgl_radio.hpp>
#include <radio_tool/radio/radio_factory.hpp>
#include <radio_tool/fw/tyt_fw_sgl.hpp>
#include <radio_tool/util/flash.hpp>

#include <math.h>
#include <iomanip>
#include <iostream>
#include <vector>

using namespace radio_tool::radio;

TYTSGLRadio::TYTSGLRadio(libusb_device_handle* h) : device(h)
{
	device.GetHID()->Setup();
}

auto TYTSGLRadio::ToString() const -> const std::string
{
	std::stringstream out;

	out << "== TYT SGL Radio Info ==" << std::endl
		<< "Radio: " << "ASD" << std::endl
		<< "RTC: " << "000";

	return out.str();
}

auto TYTSGLRadio::GetDevice() const -> const device::RadioDevice*
{
	return &device;
}

auto TYTSGLRadio::WriteFirmware(const std::string& file) -> void
{
	fw::TYTSGLFW fw;
	fw.Read(file);
	auto config = fw.GetConfig();

	auto hid = device.GetHID();
	hid->SendCommand(hid::tyt::commands::Download);
	auto rsp = hid->WaitForReply();
	if (std::equal(rsp.data.begin(), rsp.data.end(), hid::tyt::commands::Update.begin()))
	{
		hid->SendCommandAndOk(hid::tyt::OK);
	}
	else
	{
		auto download_rsp = std::string(rsp.data.begin(), rsp.data.end());
		auto update_rsp = std::string(hid::tyt::commands::Update.begin(), hid::tyt::commands::Update.end());
		std::stringstream msg("Invalid response, expected '");
		msg << update_rsp << "'" << " got '" << download_rsp << "'";

		throw new std::runtime_error(msg.str());
	}

	// send key
	hid->SendCommand(config->header.model_key);
	auto rsp_key = hid->WaitForReply();
	if (!std::equal(rsp_key.data.begin(), rsp_key.data.end(), config->header.model_key.begin()))
	{
		auto key = std::string(config->header.model_key.begin(), config->header.model_key.begin() + 5);
		auto key_rsp = std::string(rsp_key.data.begin(), rsp_key.data.end());
		std::stringstream msg("Invalid response, expected firmware key '");
		msg << key << "'" << " got '" << key_rsp << "'";

		throw new std::runtime_error(msg.str());
	}

	hid->SendCommandAndOk(hid::tyt::commands::FlashProgram);

	hid->SendCommandAndOk(config->header.radio_group);
	hid->SendCommandAndOk(config->header.radio_model);
	hid->SendCommandAndOk(config->header.protocol_version);

	hid->SendCommandAndOk(hid::tyt::commands::FlashErase);
	hid->SendCommandAndOk(hid::tyt::OK);
	hid->SendCommandAndOk(hid::tyt::commands::Program);

	constexpr auto TransferSize = 0x20u;
	constexpr auto HeaderSize = 0x06u;
	constexpr auto ChecksumBlockSize = 0x400u;

	auto buf = std::vector<uint8_t>(TransferSize + HeaderSize);
	auto binary = fw.GetDataSegments()[0];
	auto address = 0;
	auto checksumBlock = 0;
	while (address < binary.size)
	{
		auto transferSize = std::min(TransferSize, binary.size - address);
		*(uint32_t*)buf.data() = address;
		*(uint16_t*)(buf.data() + 4) = transferSize;

		auto src = binary.data.begin() + address;
		std::copy(src, src + transferSize, buf.begin() + HeaderSize);

		hid->SendCommandAndOk(buf);

		address += transferSize;
		if (address % ChecksumBlockSize == 0 || address == binary.size) {
			auto start = ChecksumBlockSize * checksumBlock;
			auto end = address;

			auto checksumCommand = std::vector<uint8_t>(hid::tyt::commands::End.size() + 5, 0xff);
			std::copy(hid::tyt::commands::End.begin(), hid::tyt::commands::End.end(), checksumCommand.begin());
			*(uint32_t*)(checksumCommand.end()._Ptr - 4) = checksum(binary.data.begin() + start, binary.data.begin() + end);
			hid->SendCommandAndOk(checksumCommand);

			checksumBlock++;
		}
	}
}

auto TYTSGLRadio::checksum(std::vector<uint8_t>::const_iterator&& begin, std::vector<uint8_t>::const_iterator&& end) const->uint32_t {
	uint32_t counter = 0;
	while (begin != end) {
		counter += *begin;
		std::advance(begin, 1);
	}
	return counter;
}
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

auto TYTSGLRadio::GetDevice() const -> const device::RadioDevice* {
	return &device;
}

auto TYTSGLRadio::WriteFirmware(const std::string& file) -> void
{
	fw::TYTSGLFW fw;
	fw.Read(file);

	auto hid = device.GetHID();
	hid->SendCommand(hid::tyt::commands::Download);
	auto rsp = hid->WaitForReply();
	if (std::equal(rsp.data.begin(), rsp.data.end(), hid::tyt::commands::Update.begin()))
	{
		hid->SendCommandAndOk(hid::tyt::OK);
	}

	//TODO: Write firmware
}
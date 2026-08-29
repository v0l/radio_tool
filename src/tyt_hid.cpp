/**
 * This file is part of radio_tool.
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
#include <radio_tool/hid/tyt_hid.hpp>
#include <radio_tool/util.hpp>

#include <stdexcept>
#include <iostream>

using namespace radio_tool::hid;

auto TYTHID::Setup() -> void
{
	auto err = 0;

	//the kernel hid driver binds these radios on Linux, hand the interface
	//over to us for the duration of the session
	if (libusb_kernel_driver_active(device, 0x00) == 1)
	{
		libusb_set_auto_detach_kernel_driver(device, 1);
	}

	//setting the configuration fails with BUSY when a driver is attached,
	//and is pointless when the device is already in the one configuration
	auto config = 0;
	err = libusb_get_configuration(device, &config);
	if (err != LIBUSB_SUCCESS)
	{
		libusb_close(device);
		throw std::runtime_error(libusb_error_name(err));
	}
	if (config != 0x01)
	{
		err = libusb_set_configuration(device, 0x01);
		if (err != LIBUSB_SUCCESS)
		{
			libusb_close(device);
			throw std::runtime_error(libusb_error_name(err));
		}
	}
	err = libusb_claim_interface(device, 0x00);
	if (err != LIBUSB_SUCCESS)
	{
		libusb_close(device);
		throw std::runtime_error(libusb_error_name(err));
	}
	//HID SET_IDLE, some bootloaders in this family stall it, which is
	//harmless as we only use the interrupt endpoints
	err = libusb_control_transfer(device, 0x21, 0x0a, 0, 0, nullptr, 0, timeout);
	if (err != LIBUSB_SUCCESS)
	{
		std::cerr << "Warning: SET_IDLE failed (" << libusb_error_name(err) << "), continuing" << std::endl;
	}
    /*
	auto buffer = (uint8_t*)malloc(64);
	auto tx = libusb_alloc_transfer(0);
	libusb_fill_interrupt_transfer(
		tx, device, TYTHID::EP_IN, buffer, 64, [](libusb_transfer* tx)
		{
			auto self = (TYTHID*)tx->user_data;
			self->OnTransfer(tx);
		},
		this, 5000);
	libusb_submit_transfer(tx);
    */
}

auto TYTHID::OnTransfer(libusb_transfer* tx) -> void
{
	if (tx->status == LIBUSB_TRANSFER_COMPLETED ||
		tx->status == LIBUSB_TRANSFER_TIMED_OUT)
	{
		{
			std::lock_guard<std::mutex> lk(signalCallback);
			this->tx = tx;
		}
		signalReady.notify_one();

		//wait again for item to be read
		{
			std::unique_lock<std::mutex> lk(signalCallback);
			auto tx_local = &this->tx;
			signalReady.wait(lk, [tx_local]
				{ return *tx_local == nullptr; });
		}
	}
	libusb_submit_transfer(tx);
}

auto TYTHID::SendCommand(const tyt::Command& cmd) -> tyt::Command
{
	if (cmd.data.size() > 0xffff)
	{
		throw std::runtime_error("Command payload is too large");
	}

	std::vector<uint8_t> payload(cmd.data.size() + 4, 0x00);

	//the header is two little endian 16 bit fields, written byte by byte so
	//the buffer does not have to be aligned for a uint16_t
	auto tx_type = (uint16_t)cmd.type;
	auto tx_len = (uint16_t)cmd.data.size();
	payload[0] = (uint8_t)(tx_type & 0xff);
	payload[1] = (uint8_t)(tx_type >> 8);
	payload[2] = (uint8_t)(tx_len & 0xff);
	payload[3] = (uint8_t)(tx_len >> 8);
	std::copy(cmd.data.begin(), cmd.data.end(), payload.begin() + 4);

	//the OUT endpoint on these radios is bulk, only the IN endpoint is
	//interrupt, sending an interrupt urb to it is rejected with EINVAL
	BulkWrite(TYTHID::EP_OUT, payload);

	//read a full packet, the device sends 64 bytes and asking for fewer
	//risks an overflow error
	auto data = InterruptRead(TYTHID::EP_IN, 0x40);
    if (data.size() < 4)
    {
        throw std::runtime_error("Short response from radio");
    }
    auto type = ((uint16_t)data[1] << 8) | data[0];
    auto len  = ((uint16_t)data[3] << 8) | data[2];
    if ((size_t)(len + 4) > data.size())
    {
        throw std::runtime_error("Radio reported more data than it sent");
    }
    return tyt::Command((tyt::CommandType)type, len,
                            std::vector<uint8_t>(data.begin() + 4, data.begin() + 4 + len));
}

auto TYTHID::SendCommand(const std::vector<uint8_t>& cmd) -> tyt::Command
{
	return SendCommand(tyt::Command(tyt::CommandType::HostToDevice, cmd.size(), cmd));
}

auto TYTHID::SendCommand(const std::vector<uint8_t>& cmd, const uint8_t& size, const uint8_t& fill) -> tyt::Command
{
	auto ncmd = std::vector<uint8_t>(size, fill);
	std::copy(cmd.begin(), cmd.end(), ncmd.begin());

	return SendCommand(ncmd);
}

auto TYTHID::WaitForReply() -> tyt::Command
{
	std::unique_lock<std::mutex> lk(signalCallback);
	auto tx_local = &this->tx;
	signalReady.wait(lk, [tx_local]
		{ return *tx_local != nullptr; });

	if (tx->status == LIBUSB_TRANSFER_COMPLETED)
	{
		if (tx->actual_length < 4)
		{
			throw std::runtime_error("Short response from radio");
		}

		//setup return, the length field comes from the device and must not be
		//trusted to index past what was actually transferred
		auto type = ((uint16_t)tx->buffer[1] << 8) | tx->buffer[0];
		auto len = ((uint16_t)tx->buffer[3] << 8) | tx->buffer[2];
		if ((int)(len + 4) > tx->actual_length)
		{
			throw std::runtime_error("Radio reported more data than it sent");
		}

		auto ret = tyt::Command((tyt::CommandType)type, len, std::vector<uint8_t>(tx->buffer + 4, tx->buffer + 4 + len));
		radio_tool::PrintHex(ret.data.begin(), ret.data.end());

		tx = nullptr;
		lk.unlock();
		signalReady.notify_one();
		return ret;
	}

	throw std::runtime_error("USB TRANSFER ERROR");
}

auto TYTHID::SendCommandAndOk(const tyt::Command& cmd) -> void
{
    auto ok = SendCommand(cmd);
	if (!(ok == tyt::OKResponse))
	{
		radio_tool::PrintHex(ok.data.begin(), ok.data.end());
		throw std::runtime_error("Invalid usb response from device");
	}
}

auto TYTHID::SendCommandAndOk(const std::vector<uint8_t>& cmd) -> void
{
    auto ok = SendCommand(cmd);
	if (!(ok == tyt::OKResponse))
	{
		radio_tool::PrintHex(ok.data.begin(), ok.data.end());
		throw std::runtime_error("Invalid usb response from device");
	}
}

auto TYTHID::SendCommandAndOk(const std::vector<uint8_t>& cmd, const uint8_t& size, const uint8_t& fill) -> void
{
    auto ok = SendCommand(cmd, size, fill);
	if (!(ok == tyt::OKResponse))
	{
		radio_tool::PrintHex(ok.data.begin(), ok.data.end());
		throw std::runtime_error("Invalid usb response from device");
	}
}
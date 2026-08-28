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
#include <radio_tool/device/serial_port.hpp>

#include <chrono>
#include <stdexcept>
#include <thread>

#ifdef _WIN32
#define WIN32_LEAN_AND_MEAN 1
#include <Windows.h>
#else
#include <errno.h>
#include <fcntl.h>
#include <termios.h>
#include <unistd.h>
#include <sys/select.h>
#include <sys/ioctl.h>
#include <cstring>
#include <algorithm>
#endif

using namespace radio_tool::device;

#ifndef _WIN32
namespace
{
	auto BaudConstant(const uint32_t &baud) -> speed_t
	{
		switch (baud)
		{
		case 1200:
			return B1200;
		case 2400:
			return B2400;
		case 4800:
			return B4800;
		case 9600:
			return B9600;
		case 19200:
			return B19200;
		case 38400:
			return B38400;
		case 57600:
			return B57600;
		case 115200:
			return B115200;
		default:
			throw std::runtime_error("Unsupported baud rate");
		}
	}
}
#endif

SerialPort::SerialPort(const std::string &port, const uint32_t &baud)
	: port(port)
#ifdef _WIN32
	  ,
	  handle(nullptr)
#else
	  ,
	  fd(-1)
#endif
{
#ifdef _WIN32
	auto path = std::string("\\\\.\\") + port;
	auto h = CreateFileA(path.c_str(), GENERIC_READ | GENERIC_WRITE, 0, NULL, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, NULL);
	if (h == INVALID_HANDLE_VALUE)
	{
		throw std::runtime_error("Failed to open port: " + port);
	}

	DCB dcb = {};
	dcb.DCBlength = sizeof(dcb);
	if (!GetCommState(h, &dcb))
	{
		CloseHandle(h);
		throw std::runtime_error("Failed to read port state: " + port);
	}
	dcb.BaudRate = baud;
	dcb.ByteSize = 8;
	dcb.StopBits = ONESTOPBIT;
	dcb.Parity = NOPARITY;
	dcb.fBinary = TRUE;
	dcb.fOutxCtsFlow = FALSE;
	dcb.fOutxDsrFlow = FALSE;
	dcb.fDtrControl = DTR_CONTROL_ENABLE;
	dcb.fRtsControl = RTS_CONTROL_ENABLE;
	dcb.fOutX = FALSE;
	dcb.fInX = FALSE;
	if (!SetCommState(h, &dcb))
	{
		CloseHandle(h);
		throw std::runtime_error("Failed to configure port: " + port);
	}

	//timeouts are applied per read call in Read()
	COMMTIMEOUTS to = {};
	to.ReadIntervalTimeout = 0;
	to.ReadTotalTimeoutConstant = 1000;
	to.ReadTotalTimeoutMultiplier = 0;
	to.WriteTotalTimeoutConstant = 1000;
	to.WriteTotalTimeoutMultiplier = 0;
	SetCommTimeouts(h, &to);

	handle = h;
#else
	auto f = open(port.c_str(), O_RDWR | O_NOCTTY | O_NONBLOCK);
	if (f < 0)
	{
		throw std::runtime_error("Failed to open port: " + port + " (" + strerror(errno) + ")");
	}

	struct termios tty = {};
	if (tcgetattr(f, &tty) != 0)
	{
		close(f);
		throw std::runtime_error("Failed to read TTY attributes: " + port);
	}

	cfmakeraw(&tty);
	cfsetispeed(&tty, BaudConstant(baud));
	cfsetospeed(&tty, BaudConstant(baud));

	tty.c_cflag |= (CLOCAL | CREAD);
	tty.c_cflag &= ~(PARENB | PARODD | CSTOPB | CRTSCTS);
	tty.c_cflag = (tty.c_cflag & ~CSIZE) | CS8;
	tty.c_cc[VMIN] = 0;
	tty.c_cc[VTIME] = 0;

	if (tcsetattr(f, TCSANOW, &tty) != 0)
	{
		close(f);
		throw std::runtime_error("Failed to set TTY attributes: " + port);
	}

	//assert DTR/RTS, the same as pyserial does by default
	int status = 0;
	if (ioctl(f, TIOCMGET, &status) == 0)
	{
		status |= (TIOCM_DTR | TIOCM_RTS);
		ioctl(f, TIOCMSET, &status);
	}

	tcflush(f, TCIOFLUSH);
	fd = f;
#endif
}

SerialPort::~SerialPort()
{
#ifdef _WIN32
	if (handle != nullptr)
	{
		CloseHandle((HANDLE)handle);
	}
#else
	if (fd >= 0)
	{
		close(fd);
	}
#endif
}

auto SerialPort::Sleep(const uint32_t &ms) -> void
{
	std::this_thread::sleep_for(std::chrono::milliseconds(ms));
}

auto SerialPort::Write(const std::vector<uint8_t> &data) const -> void
{
	size_t sent = 0;
	while (sent < data.size())
	{
#ifdef _WIN32
		DWORD written = 0;
		if (!WriteFile((HANDLE)handle, data.data() + sent, (DWORD)(data.size() - sent), &written, NULL))
		{
			throw std::runtime_error("Failed to write to port: " + port);
		}
		if (written == 0)
		{
			throw std::runtime_error("Timed out writing to port: " + port);
		}
		sent += written;
#else
		auto written = write(fd, data.data() + sent, data.size() - sent);
		if (written < 0)
		{
			if (errno == EAGAIN || errno == EINTR)
			{
				continue;
			}
			throw std::runtime_error("Failed to write to port: " + port);
		}
		sent += written;
#endif
	}
}

auto SerialPort::WriteSlow(const std::vector<uint8_t> &data, const uint32_t &delay_ms) const -> void
{
	for (const auto &b : data)
	{
		Write({b});
		Sleep(delay_ms);
	}
}

auto SerialPort::Read(const size_t &len, const uint32_t &timeout_ms) const -> std::vector<uint8_t>
{
	std::vector<uint8_t> ret;
	ret.reserve(len);

	auto deadline = std::chrono::steady_clock::now() + std::chrono::milliseconds(timeout_ms);
	while (ret.size() < len)
	{
		auto now = std::chrono::steady_clock::now();
		if (now >= deadline)
		{
			break;
		}
		auto remain = std::chrono::duration_cast<std::chrono::milliseconds>(deadline - now).count();

		uint8_t buf[64] = {};
		auto want = std::min(len - ret.size(), sizeof(buf));

#ifdef _WIN32
		COMMTIMEOUTS to = {};
		to.ReadIntervalTimeout = 0;
		to.ReadTotalTimeoutConstant = (DWORD)remain;
		to.ReadTotalTimeoutMultiplier = 0;
		to.WriteTotalTimeoutConstant = 1000;
		SetCommTimeouts((HANDLE)handle, &to);

		DWORD got = 0;
		if (!ReadFile((HANDLE)handle, buf, (DWORD)want, &got, NULL))
		{
			throw std::runtime_error("Failed to read from port: " + port);
		}
		if (got == 0)
		{
			break;
		}
		ret.insert(ret.end(), buf, buf + got);
#else
		fd_set rd;
		FD_ZERO(&rd);
		FD_SET(fd, &rd);

		struct timeval tv = {};
		tv.tv_sec = remain / 1000;
		tv.tv_usec = (remain % 1000) * 1000;

		auto sel = select(fd + 1, &rd, nullptr, nullptr, &tv);
		if (sel < 0)
		{
			if (errno == EINTR)
			{
				continue;
			}
			throw std::runtime_error("Failed to read from port: " + port);
		}
		if (sel == 0)
		{
			break; //timeout
		}

		auto got = read(fd, buf, want);
		if (got < 0)
		{
			if (errno == EAGAIN || errno == EINTR)
			{
				continue;
			}
			throw std::runtime_error("Failed to read from port: " + port);
		}
		if (got == 0)
		{
			continue;
		}
		ret.insert(ret.end(), buf, buf + got);
#endif
	}

	return ret;
}

auto SerialPort::ReadExact(const size_t &len, const uint32_t &timeout_ms, const std::string &what) const -> std::vector<uint8_t>
{
	auto ret = Read(len, timeout_ms);
	if (ret.size() != len)
	{
		throw std::runtime_error("Timed out reading " + what + " (wanted " + std::to_string(len) +
								 " bytes, got " + std::to_string(ret.size()) + ")");
	}
	return ret;
}

auto SerialPort::FlushInput() const -> void
{
#ifdef _WIN32
	PurgeComm((HANDLE)handle, PURGE_RXCLEAR | PURGE_TXCLEAR);
#else
	tcflush(fd, TCIOFLUSH);
#endif
}

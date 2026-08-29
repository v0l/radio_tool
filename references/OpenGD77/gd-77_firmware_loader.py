#!/usr/bin/env python
# -*- coding: utf-8 -*-
################################################################################################################################################
#
# GD-77 Firmware uploader. By Roger VK3KYY
# 
#
# This script has only been tested on Windows and Linux, it may or may not work on OSX
#
# On Windows,..
# the driver the system installs for the GD-77, which is the HID driver, needs to be replaced by the LibUSB-win32 using Zadig
# for USB device with idVendor=0x15a2, idProduct=0x0073
# Once this driver is installed the CPS and official firmware loader will no longer work as they can't find the device
# To use the CPS etc again, use the DeviceManager to uninstall the driver associated with idVendor=0x15a2, idProduct=0x0073 (this will appear as a libusb-win32 device)
# Then unplug the GD-77 and reconnect, and the HID driver will be re-installed
#
#
# On Linux, depending of you distro, you need to install a special udev rule to automatically unbind the USB HID device to usbhid driver.
#
#
# You need to install future if you're running python2: 
#    debian like: sudo apt-get install python-future
#    or: pip install future
#
# You also need python-usb or python3-usb
#
################################################################################################################################################
from __future__ import print_function
import usb
import getopt, sys
import ntpath
import os.path
from array import array

# Globals
responseOK = [0x41]


########################################################################
# Utilities to dump hex for testing
########################################################################
def hexdump(buf):
    cbuf = ""
    for b in buf:
        cbuf = cbuf + "0x%0.2X " % ord(b)
    return cbuf

def hexdumpArray(buf):
    cbuf = ""
    for b in buf:
        cbuf = cbuf + "0x%0.2X " % b
    return cbuf

def hexdumpArray2(buf):
    cbuf = ""
    for b in buf:
        cbuf = cbuf + "%0.2X-" % b
    return cbuf[:-1]

########################################################################
# Send the data packet to the GD-77 and confirm the response is correct
########################################################################
def sendAndCheckResponse(dev, cmd, resp):
    USB_WRITE_ENDPOINT  = 0x02
    USB_READ_ENDPOINT   = 0x81
    TRANSFER_LENGTH     = 38
    zeroPad = [0x0] * TRANSFER_LENGTH
    headerData = [0x0] * 4
        
    headerData[0] = 1
    headerData[1] = 0
    headerData[2] = ((len(cmd) >> 0) & 0xff)
    headerData[3] = ((len(cmd) >> 8) & 0xff)

    if (len(resp) < TRANSFER_LENGTH):
        resp = resp + zeroPad[0:TRANSFER_LENGTH - len(resp)]

    cmd = headerData + cmd
    
    #print("TX: " + hexdumpArray2(cmd))

    ret = dev.write(USB_WRITE_ENDPOINT, cmd)
    ret = dev.read(USB_READ_ENDPOINT, TRANSFER_LENGTH + 4, 5000)
    expected = array("B", resp)
    ##print("RX: " + hexdumpArray2(ret[4:]))

    if (expected == ret[4:]):
        return True
    else:
        print("Error read returned " + str(ret))
        return False

 
##############################
# Create checksum data packet
##############################
def createChecksumData(buf, startAddress, endAddress):
    #checksum data starts with a small header, followed by the 32 bit checksum value, least significant byte first
    checkSumData = [ 0x45, 0x4e, 0x44, 0xff, 0xDE, 0xAD, 0xBE, 0xEF ]
    cs = 0
    
    for i in range(startAddress, endAddress):
        if (sys.version_info > (3, 0)):
            cs = cs + buf[i]
        else:
            cs = cs + ord(buf[i]) #the file data seems to be a string, hence the ord() function is needed to convert it to an integer
     
    checkSumData[4] = (cs % 256) & 0xff
    checkSumData[5] = ((cs >> 8) % 256) & 0xff
    checkSumData[6] = ((cs >> 16) % 256) & 0xff
    checkSumData[7] = ((cs >> 24) % 256) & 0xff
    return checkSumData


def updateBlockAddressAndLength(buf, address, length):
	buf[5] = ((length) % 256) & 0xff
	buf[4] = ((length >> 8) % 256) & 0xff
	buf[3] = ((address) % 256) & 0xff
	buf[2] = ((address >> 8) % 256) & 0xff
	buf[1] = ((address >> 16) % 256) & 0xff
	buf[0] = ((address >> 24) % 256) & 0xff


#####################################################
# Open firmware file on disk and sent it to the GD-77
###########################################b##########
def sendFileData(fileBuf, dev):
    dataHeader = [0x00] * (0x20 + 0x06)
    BLOCK_LENGTH = 1024 #1k
    DATA_TRANSFER_SIZE = 0x20
    checksumStartAddress = 0
    address = 0
         
    fileLength = len(fileBuf)
    totalBlocks = (fileLength // BLOCK_LENGTH) + 1

    while address < fileLength:
        if ((address % BLOCK_LENGTH) == 0):
            checksumStartAddress = address
            
        updateBlockAddressAndLength(dataHeader, address, DATA_TRANSFER_SIZE)
        
        if ((address + DATA_TRANSFER_SIZE) < fileLength):
            
            for i in range(DATA_TRANSFER_SIZE):
                if (sys.version_info > (3, 0)):
                    dataHeader[6 + i] = fileBuf[address + i]
                else:
                    dataHeader[6 + i] = ord(fileBuf[address + i])  ## + [ord(i) for i in input[address:(address + DATA_TRANSFER_SIZE)]]
 
            if  (sendAndCheckResponse(dev, dataHeader, responseOK) == False):
                print("Error sending data")
                return False
                break
            
            address = address + DATA_TRANSFER_SIZE
            
            if ((address % 0x400) == 0):
                print("\r - Sent block " + str(address // BLOCK_LENGTH) + " of "+ str(totalBlocks), end='')
                sys.stdout.flush()

                if (sendAndCheckResponse(dev, createChecksumData(fileBuf, checksumStartAddress, address), responseOK) == False):
                    print("Error sending checksum")
                    return False
                    break
                
        else:
            print("\r - Sending last block                   ", end='')
            sys.stdout.flush()
            
            DATA_TRANSFER_SIZE = fileLength - address

            updateBlockAddressAndLength(dataHeader, address, DATA_TRANSFER_SIZE)
            
            for i in range(DATA_TRANSFER_SIZE):
                if (sys.version_info > (3, 0)):
                    dataHeader[6 + i] = fileBuf[address + i]
                else:
                    dataHeader[6 + i] = ord(fileBuf[address + i])  ## + [ord(i) for i in input[address:(address + DATA_TRANSFER_SIZE)]]
            
            if (sendAndCheckResponse(dev, dataHeader, responseOK) == False):
                print("Error sending data")
                return False
                break
            
            address = address + DATA_TRANSFER_SIZE

            if (sendAndCheckResponse(dev, createChecksumData(fileBuf, checksumStartAddress, address), responseOK) == False):
                print("Error sending checksum")
                return False
                break

            print("")
    return True

###########################################################################################################################################
# Send commands to the GD-77 to verify we are the updater, prepare to program including erasing the internal program flash memory
###########################################################################################################################################
def sendInitialCommands(dev, encodeKey):
    commandLetterA      =[ 0x41] #A
    command0            =[[0x44,0x4f,0x57,0x4e,0x4c,0x4f,0x41,0x44],[0x23,0x55,0x50,0x44,0x41,0x54,0x45,0x3f]] # DOWNLOAD
    command1            =[commandLetterA,responseOK] 
    command2            =[[0x44, 0x56, 0x30, 0x31, (0x61 + 0x00), (0x61 + 0x0C), (0x61 + 0x0D), (0x61 + 0x01)],[0x44, 0x56, 0x30, 0x31]] #.... DV01enhi (DV01enhi comes from deobfuscated sgl file)
    command3            =[[0x46, 0x2d, 0x50, 0x52, 0x4f, 0x47, 0xff, 0xff],responseOK] #... F-PROG..
    command4            =[[0x53, 0x47, 0x2d, 0x4d, 0x44, 0x2d, 0x37, 0x36, 0x30, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],responseOK] #SG-MD-760
    command5            =[[0x4d, 0x44, 0x2d, 0x37, 0x36, 0x30, 0xff, 0xff],responseOK] #MD-760..
    command6            =[[0x56, 0x31, 0x2e, 0x30, 0x30, 0x2e, 0x30, 0x31],responseOK] #V1.00.01
    commandErase        =[[0x46, 0x2d, 0x45, 0x52, 0x41, 0x53, 0x45, 0xff],responseOK] #F-ERASE
    commandPostErase    =[commandLetterA,responseOK] 
    commandProgram      =[[0x50, 0x52, 0x4f, 0x47, 0x52, 0x41, 0x4d, 0xf],responseOK] #PROGRAM
    commands            =[command0,command1,command2,command3,command4,command5,command6,commandErase,commandPostErase,commandProgram]
    commandNames        =["Sending Download command", "Sending ACK", "Sending encryption key", "Sending F-PROG command", "Sending radio modem number", "Sending radio modem number 2", "Sending version", "Sending erase command", "Send post erase command", "Sending Program command"]
    commandNumber = 0
    
    # Buffer.BlockCopy(encodeKey, 0, command2[0], 4, 4);
    command2 = encodeKey + command2

    # Send the commands which the GD-77 expects before the start of the data
    while commandNumber < len(commands):
        print(" - " + commandNames[commandNumber])

        if sendAndCheckResponse(dev, commands[commandNumber][0], commands[commandNumber][1]) == False:
            print("Error sending command")
            return False
            break
        commandNumber = commandNumber + 1
    return True

###########################################################################################################################################
#
###########################################################################################################################################
def checkForSGLAndReturnEncryptedData(fileBuf, encodeKey):
    header_tag = list("SGL!")

    if (sys.version_info > (3, 0)):
        buf_in_4 = list("".join(map(chr, fileBuf[0:4])))
    else:
        buf_in_4 = list(fileBuf[0:4])
    
    if buf_in_4 == header_tag:
        # read and decode offset and xor tag
        buf_in_4 = list(fileBuf[0x000C : 0x000C + 4])
        
        for i in range(0, 4):
                if (sys.version_info > (3, 0)):
                    buf_in_4[i] = buf_in_4[i] ^ ord(header_tag[i])
                else:
                    buf_in_4[i] = ord(buf_in_4[i]) ^ ord(header_tag[i])
            
        offset = buf_in_4[0] + 256 * buf_in_4[1]
        xor_data = [ buf_in_4[2], buf_in_4[3] ]
        
	    # read and decode part of the header
        buf_in_512 = list(fileBuf[offset + 0x0006 : offset + 0x0006 + 512])
    
        xor_idx = 0;
        for i in range(0, 512):
            if (sys.version_info > (3, 0)):
                buf_in_512[i] = buf_in_512[i] ^ xor_data[xor_idx]
            else:
                buf_in_512[i] = ord(buf_in_512[i]) ^ xor_data[xor_idx]
            
            xor_idx += 1
            if xor_idx == 2:
                xor_idx = 0

        #
        encodeKey = buf_in_512[0x005D : 0x005D + 4]

        # extract length
        length1 = buf_in_512[0x0000]
        length2 = buf_in_512[0x0001]
        length3 = buf_in_512[0x0002]
        length4 = buf_in_512[0x0003]
        length = (length4 << 24) + (length3 << 16) + (length2 << 8) + length1
        
	    # extract encoded raw firmware
        retBuf = [0x00] * length;
        retBuf = fileBuf[len(fileBuf) - length : len(fileBuf) - length + len(retBuf) ]

        return retBuf
    
    print("ERROR: SGL! header is missing.")
    return None

###########################################################################################################################################
#
###########################################################################################################################################
def usage():
    print("Usage:")
    print("       " + ntpath.basename(sys.argv[0]) + " [OPTION]")
    print("")
    print("    -h, --help                     : Display this help text")
    print("    -f, --firmware=<filename.sgl>  : Flash <filename.sgl> instead of default file \"GD-77_V3.1.2.sgl\"")
    print("")

#####################################################
# Main function.
#####################################################
def main():
    sglFile = "firmware.sgl"
    dev = usb.core.find(idVendor=0x15a2, idProduct=0x0073)
    if (dev):
        encodeKey = [ (0x61 + 0x00), (0x61 + 0x0C), (0x61 + 0x0D), (0x61 + 0x01) ]

        # Command line argument parsing
        try:                                
            opts, args = getopt.getopt(sys.argv[1:], "hf:", ["help", "firmware="])
        except getopt.GetoptError as err:
            print(str(err))
            usage()
            sys.exit(2)
        
        for opt, arg in opts:
            if opt in ("-h", "--help"):
                usage()
                sys.exit(2)
            elif opt in ("-f", "--firmware"):
                sglFile = arg
            else:
                assert False, "Unhandled option"

        if (os.path.isfile(sglFile) == False):
            print("Firmware file \"" + sglFile + "\" is missing.")
            sys.exit(2)

        # Needed on Linux
        try:
            if dev.is_kernel_driver_active(0):
                dev.detach_kernel_driver(0)
        except NotImplementedError as e:
            pass
        
        print("Now flashing your GD-77 with \"" + sglFile + "\"")
        
        #seems to be needed for the usb to work !
        dev.set_configuration()

        with open(sglFile, 'rb') as f:
            fileBuf = f.read()
            
        # Chech firmware        
        filename, file_extension = os.path.splitext(sglFile)

        if file_extension == ".sgl":
            ## Could be a SGL file !
            fileBuf = checkForSGLAndReturnEncryptedData(fileBuf, encodeKey)
            
            if fileBuf == None:
                print("Error. Missing SGL in .sgl file header")
                sys.exit(-1)
                
            print("Firmware file confirmed as SGL")
        else:
            print("Firmware file is an unencrypted binary. Exiting")
            sys.exit(-10)

        if len(fileBuf) > 0x7b000:
            print("Error. Firmware file too large.")
            sys.exit(-2)

        if (sendInitialCommands(dev, encodeKey) == True):
            if (sendFileData(fileBuf, dev) == True):
                print("Firmware update complete. Please reboot the GD-77")
            else:
                print("Error while sending data")
        else:
            print("Error while sending initial commands")
       
        usb.util.dispose_resources(dev) #free up the USB device
        
    else:
        print("Cant find GD-77")


## Run the program
main() 

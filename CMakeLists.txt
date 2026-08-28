cmake_minimum_required(VERSION 3.10.0)
project(radio_tool VERSION 0.3.0)

option(BUILD_TESTING "Enable tests" OFF)
option(BUILD_BLE "Enable Bluetooth LE support (fetches SimpleBLE)" ON)

if(NOT CMAKE_BUILD_TYPE)
  set(CMAKE_BUILD_TYPE Release)
endif()
string(TOLOWER ${CMAKE_BUILD_TYPE} CMAKE_BUILD_TYPE_LOWER)

#only Visual Studio generators accept a platform, setting it unconditionally
#breaks any sub-build using Unix Makefiles
if(NOT CMAKE_GENERATOR_PLATFORM AND CMAKE_GENERATOR MATCHES "Visual Studio")
  set(CMAKE_GENERATOR_PLATFORM x64)
endif()

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

list(APPEND CMAKE_MODULE_PATH "${CMAKE_CURRENT_SOURCE_DIR}/cmake")

#Git version
include(GetGitRevisionDescription)
get_git_head_revision(GIT_REFSPEC GIT_SHA1)
git_describe(GIT_SHA1_SHORT "--always")
configure_file("${CMAKE_CURRENT_SOURCE_DIR}/src/version.cpp.in" "${CMAKE_CURRENT_BINARY_DIR}/src/version.cpp" @ONLY)

#FindLIBUSB
if (${CMAKE_CXX_COMPILER_ID} STREQUAL GNU OR ${CMAKE_CXX_COMPILER_ID} STREQUAL AppleClang)
  find_package(LibUSB REQUIRED)
  set(CMAKE_CXX_FLAGS "-Wall -Wextra")
  set(CMAKE_CXX_FLAGS_DEBUG "-g")
  set(CMAKE_CXX_FLAGS_RELEASE "-O3")
elseif (${CMAKE_CXX_COMPILER_ID} STREQUAL MSVC)
  set(VCPKG_INCLUDE_DIR $ENV{VCPKG_ROOT}\\installed\\${CMAKE_GENERATOR_PLATFORM}-windows\\include)
  if(${CMAKE_BUILD_TYPE_LOWER} STREQUAL release)
	set(VCPKG_LIB_DIR $ENV{VCPKG_ROOT}\\installed\\${CMAKE_GENERATOR_PLATFORM}-windows\\lib)
  elseif (${CMAKE_BUILD_TYPE_LOWER} STREQUAL debug)
    set(VCPKG_LIB_DIR $ENV{VCPKG_ROOT}\\installed\\${CMAKE_GENERATOR_PLATFORM}-windows\\debug\\lib)
  endif()
  message("-- VCPKG include dir: ${VCPKG_INCLUDE_DIR}")
  message("-- VCPKG lib dir: ${VCPKG_LIB_DIR}")
  add_definitions(-DNOMINMAX -DVC_EXTRALEAN -DWIN32_LEAN_AND_MEAN)
endif()

#SimpleBLE, for radios which clone over Bluetooth LE instead of a cable
if(BUILD_BLE)
  include(FetchContent)
  FetchContent_Declare(
    simpleble
    GIT_REPOSITORY https://github.com/simpleble/simpleble.git
    GIT_TAG v1.1.0
    GIT_SHALLOW TRUE
    SOURCE_SUBDIR simpleble
  )
  FetchContent_MakeAvailable(simpleble)
endif()

if(BUILD_TESTING)
  include(CTest)
  enable_testing()
  add_subdirectory(test)
endif()

set(ALL_SRC 
    src/radio_tool.cpp
    src/dfu.cpp
    src/h8sx.cpp
    src/radio_factory.cpp
    src/usb_radio_factory.cpp
    src/serial_radio_factory.cpp
    src/tyt_radio.cpp
    src/ymodem_device.cpp
    src/tyt_dfu.cpp
    src/tyt_fw.cpp
    src/tyt_fw_sgl.cpp
    src/cs_fw.cpp
    src/ailunce_radio.cpp
    src/ailunce_fw.cpp
    src/yaesu_radio.cpp
    src/yaesu_fw.cpp
    src/fymodem.c
    src/rdt.cpp
    src/uv5r_codeplug.cpp
    src/uv5r_radio.cpp
    src/uv17pro_radio.cpp
    src/byte_stream.cpp
    src/serial_port.cpp
    src/hid.cpp
    src/tyt_hid.cpp
    src/tyt_sgl_radio.cpp
    "${CMAKE_CURRENT_BINARY_DIR}/src/version.cpp"
)

file(GLOB_RECURSE ALL_HEADERS include false "*.h*")

if(BUILD_BLE)
  list(APPEND ALL_SRC src/ble_port.cpp)
endif()

add_library(radiotool ${ALL_SRC} ${ALL_HEADERS})
target_include_directories(radiotool PUBLIC include)

if(BUILD_BLE)
  target_compile_definitions(radiotool PUBLIC RADIO_TOOL_BLE)
  target_link_libraries(radiotool simpleble::simpleble)
endif()

add_executable(radio_tool src/radio_tool.cpp)
target_include_directories(radio_tool PUBLIC include)
target_link_libraries(radio_tool radiotool)

if (${CMAKE_CXX_COMPILER_ID} STREQUAL GNU OR ${CMAKE_CXX_COMPILER_ID} STREQUAL AppleClang)
  target_link_libraries(radiotool pthread)
  target_link_libraries(radiotool PkgConfig::LibUSB)
  target_link_libraries(radio_tool PkgConfig::LibUSB)
  target_include_directories(radiotool PUBLIC ${LibUSB_INCLUDEDIR})
  target_include_directories(radio_tool PUBLIC ${LibUSB_INCLUDEDIR})
elseif (${CMAKE_CXX_COMPILER_ID} STREQUAL MSVC)
  target_include_directories(radiotool PUBLIC ${VCPKG_INCLUDE_DIR})
  target_link_directories(radiotool PRIVATE ${VCPKG_LIB_DIR})
  target_include_directories(radio_tool PUBLIC ${VCPKG_INCLUDE_DIR})
  target_link_directories(radio_tool PRIVATE ${VCPKG_LIB_DIR})
  
  target_link_libraries(radiotool libusb-1.0)
  target_link_libraries(radio_tool libusb-1.0)
endif()

install(TARGETS radio_tool RUNTIME DESTINATION bin)

set(CPACK_PROJECT_NAME ${PROJECT_NAME})
set(CPACK_PROJECT_VERSION ${PROJECT_VERSION})
set(CPACK_PACKAGE_DESCRIPTION "Universal radio firmware tool")
set(CPACK_PACKAGE_HOMEPAGE_URL "https://github.com/v0l/radio_tool")
set(CPACK_PACKAGE_CONTACT "v0l <radio_tool@v0l.io>")
set(CPACK_RESOURCE_FILE_LICENSE "${PROJECT_SOURCE_DIR}/LICENSE")
if(${CMAKE_CXX_COMPILER_ID} STREQUAL AppleClang)
  set(CPACK_GENERATOR "DragNDrop")
else()
  set(CPACK_GENERATOR "DEB" "TGZ" "TXZ")
endif()
set(CPACK_PACKAGE_CHECKSUM SHA256)
set(CPACK_STRIP_FILES true)

set(CPACK_DEBIAN_PACKAGE_ARCHITECTURE "amd64")
set(CPACK_DEBIAN_PACKAGE_DEPENDS "libusb-1.0-0, libdbus-1-3")
set(CPACK_DEBIAN_PACKAGE_SECTION "hamradio")
set(CPACK_DEBIAN_COMPRESSION_TYPE "xz")
include(CPack)

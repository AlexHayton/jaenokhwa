#!/bin/sh

#
# Copyright 2024 Alex Hayton / The Jaenokhwa Contributors
# Copyright 2022 l1npengtul <l1npengtul@protonmail.com> / The Nokhwa Contributors
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
#

#
# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.
#

mkdir public
mkdir dist
cd ../../
sh make-npm.sh
cd examples/jscam || return
npm install --save-dev webpack webpack-cli webpack-dev-server
npm install --save ../../nokhwajs
npm run build

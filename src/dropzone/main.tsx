// SPDX-License-Identifier: GPL-3.0-or-later
import {StrictMode} from 'react';
import {createRoot} from 'react-dom/client';
import '@astryxdesign/core/reset.css';
import '../index.css';
import './dropzone.css';
import {DropZone} from './DropZone';

createRoot(document.getElementById('root')!).render(<StrictMode><DropZone /></StrictMode>);

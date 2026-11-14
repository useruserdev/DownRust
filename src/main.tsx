// SPDX-License-Identifier: GPL-3.0-or-later
import {StrictMode} from 'react';
import {createRoot} from 'react-dom/client';
import '@astryxdesign/core/reset.css';
import './index.css';
import './scrollbars.css';
import {App} from './app/App';

createRoot(document.getElementById('root')!).render(<StrictMode><App /></StrictMode>);

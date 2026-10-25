import { mount } from 'svelte'
import Dev from './Dev.svelte'
import { themes } from '../themes'
for (const [k, v] of Object.entries(themes.default.vars)) document.documentElement.style.setProperty(k, v)
mount(Dev, { target: document.getElementById('app')! })

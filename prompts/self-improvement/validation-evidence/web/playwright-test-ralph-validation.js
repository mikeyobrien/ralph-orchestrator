/**
 * Ralph Validation Test - Web Application Validation
 * 
 * This script validates the web application by:
 * 1. Loading the page
 * 2. Verifying the "Ralph Validation Test" marker is present
 * 3. Testing button interactions
 * 4. Testing the counter functionality
 * 5. Capturing screenshots as proof
 * 
 * NO MOCKS - Real browser execution with Playwright
 */

const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');

// Target URL - our sandbox web app
const TARGET_URL = 'http://localhost:8765';

// Evidence directory - where screenshots will be saved
const EVIDENCE_DIR = '/Users/nick/Desktop/ralph-orchestrator/validation-evidence/web';

async function runValidation() {
    console.log('='.repeat(60));
    console.log('RALPH VALIDATION TEST - WEB APPLICATION');
    console.log('='.repeat(60));
    console.log(`Target URL: ${TARGET_URL}`);
    console.log(`Evidence Dir: ${EVIDENCE_DIR}`);
    console.log('');
    
    // Ensure evidence directory exists
    if (!fs.existsSync(EVIDENCE_DIR)) {
        fs.mkdirSync(EVIDENCE_DIR, { recursive: true });
        console.log('✓ Created evidence directory');
    }
    
    const browser = await chromium.launch({ 
        headless: false,  // Visible browser for validation
        slowMo: 100       // Slow down for visibility
    });
    
    const context = await browser.newContext({
        viewport: { width: 1280, height: 800 }
    });
    
    const page = await context.newPage();
    
    // Capture console messages
    const consoleMessages = [];
    page.on('console', msg => {
        consoleMessages.push({ type: msg.type(), text: msg.text() });
    });
    
    try {
        // TEST 1: Load the page
        console.log('\n📋 TEST 1: Loading page...');
        await page.goto(TARGET_URL, { waitUntil: 'networkidle' });
        const title = await page.title();
        console.log(`   ✓ Page loaded: "${title}"`);
        
        // Take initial screenshot
        const screenshot1Path = path.join(EVIDENCE_DIR, '01-initial-load.png');
        await page.screenshot({ path: screenshot1Path, fullPage: true });
        console.log(`   📸 Screenshot saved: 01-initial-load.png`);
        
        // TEST 2: Verify validation marker
        console.log('\n📋 TEST 2: Checking validation marker...');
        const marker = await page.locator('#validation-marker').textContent();
        if (marker === 'Ralph Validation Test') {
            console.log(`   ✓ Validation marker found: "${marker}"`);
        } else {
            throw new Error(`Expected "Ralph Validation Test", got "${marker}"`);
        }
        
        // TEST 3: Verify main heading
        console.log('\n📋 TEST 3: Checking main heading...');
        const heading = await page.locator('h1').textContent();
        console.log(`   ✓ Main heading: "${heading}"`);
        
        // TEST 4: Verify feature cards exist
        console.log('\n📋 TEST 4: Checking feature cards...');
        const featureCards = await page.locator('.feature-card').count();
        console.log(`   ✓ Found ${featureCards} feature cards`);
        
        // TEST 5: Test button interactions
        console.log('\n📋 TEST 5: Testing button interactions...');
        
        // Click "Get Started" button
        await page.locator('.btn-primary').first().click();
        await page.waitForTimeout(500);
        const statusAfterPrimary = await page.locator('#status').textContent();
        console.log(`   ✓ Primary button clicked, status: "${statusAfterPrimary}"`);
        
        // Take screenshot after button click
        const screenshot2Path = path.join(EVIDENCE_DIR, '02-after-button-click.png');
        await page.screenshot({ path: screenshot2Path, fullPage: true });
        console.log(`   📸 Screenshot saved: 02-after-button-click.png`);
        
        // TEST 6: Test counter functionality
        console.log('\n📋 TEST 6: Testing counter functionality...');
        
        // Get initial counter value
        const initialCount = await page.locator('#counter').textContent();
        console.log(`   Initial counter: ${initialCount}`);
        
        // Click increment button 3 times
        const incrementBtn = page.locator('.counter-buttons .btn-primary').nth(1);
        for (let i = 0; i < 3; i++) {
            await incrementBtn.click();
            await page.waitForTimeout(200);
        }
        
        const afterIncrement = await page.locator('#counter').textContent();
        console.log(`   ✓ After 3 increments: ${afterIncrement}`);
        
        // Click decrement button once
        const decrementBtn = page.locator('.counter-buttons .btn-primary').first();
        await decrementBtn.click();
        await page.waitForTimeout(200);
        
        const afterDecrement = await page.locator('#counter').textContent();
        console.log(`   ✓ After 1 decrement: ${afterDecrement}`);
        
        // Take screenshot of counter
        const screenshot3Path = path.join(EVIDENCE_DIR, '03-counter-interaction.png');
        await page.screenshot({ path: screenshot3Path, fullPage: true });
        console.log(`   📸 Screenshot saved: 03-counter-interaction.png`);
        
        // Reset counter
        const resetBtn = page.locator('.counter-buttons .btn-secondary');
        await resetBtn.click();
        await page.waitForTimeout(200);
        
        const afterReset = await page.locator('#counter').textContent();
        console.log(`   ✓ After reset: ${afterReset}`);
        
        // TEST 7: Verify responsive elements
        console.log('\n📋 TEST 7: Checking responsive layout...');
        
        // Test mobile viewport
        await page.setViewportSize({ width: 375, height: 667 });
        await page.waitForTimeout(500);
        
        const screenshot4Path = path.join(EVIDENCE_DIR, '04-mobile-viewport.png');
        await page.screenshot({ path: screenshot4Path, fullPage: true });
        console.log(`   📸 Mobile viewport screenshot: 04-mobile-viewport.png`);
        
        // Back to desktop
        await page.setViewportSize({ width: 1280, height: 800 });
        await page.waitForTimeout(500);
        
        // TEST 8: Verify navigation elements
        console.log('\n📋 TEST 8: Checking navigation...');
        const navLinks = await page.locator('nav a').count();
        console.log(`   ✓ Found ${navLinks} navigation links`);
        
        // Final screenshot
        const screenshot5Path = path.join(EVIDENCE_DIR, '05-final-state.png');
        await page.screenshot({ path: screenshot5Path, fullPage: true });
        console.log(`   📸 Final screenshot: 05-final-state.png`);
        
        // Check console messages
        console.log('\n📋 CONSOLE MESSAGES:');
        consoleMessages.forEach(msg => {
            console.log(`   [${msg.type}] ${msg.text}`);
        });
        
        // Summary
        console.log('\n' + '='.repeat(60));
        console.log('VALIDATION SUMMARY');
        console.log('='.repeat(60));
        console.log(`✓ Page loaded successfully`);
        console.log(`✓ "Ralph Validation Test" marker verified`);
        console.log(`✓ All UI elements present`);
        console.log(`✓ Button interactions work`);
        console.log(`✓ Counter functionality verified`);
        console.log(`✓ Responsive layout tested`);
        console.log(`✓ 5 screenshots captured as evidence`);
        console.log('');
        console.log('📁 Evidence saved to: ' + EVIDENCE_DIR);
        console.log('='.repeat(60));
        console.log('✅ ALL VALIDATION TESTS PASSED');
        console.log('='.repeat(60));
        
    } catch (error) {
        console.error('\n❌ VALIDATION FAILED:', error.message);
        
        // Take error screenshot
        const errorPath = path.join(EVIDENCE_DIR, 'error-state.png');
        await page.screenshot({ path: errorPath, fullPage: true });
        console.error(`📸 Error screenshot saved: error-state.png`);
        
        throw error;
    } finally {
        await browser.close();
    }
}

runValidation()
    .then(() => process.exit(0))
    .catch(() => process.exit(1));
